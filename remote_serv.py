import logging
import spidev
import time
import socket
import select
import json
import pigpio
import threading

from luma.oled.device import ssd1306
from luma.core.interface.serial import i2c
from luma.core.render import canvas

logger = logging.getLogger(__name__)
logging.basicConfig(filename="/home/pi/remote.log", encoding="utf-8", level=logging.INFO)

logger.info("*****************************")
logger.info("* Starting PIZ Remote Now ! *")
logger.info("*****************************")
logger.info("")

def now():
    return int(time.time() * 1000.0)

class CircBuffer:
    def __init__(self, max_size=10):
        self.buffer = [0] * max_size
        self.size = 0
        self.max_size = max_size
        self.index = 0

    def add(self, value):
        self.buffer[self.index] = value
        self.index = (self.index + 1) % self.max_size
        if self.size < self.max_size:
            self.size += 1

    def average(self):
        if self.size == 0:
            return 0
        return sum(self.buffer[:self.size]) / self.size

def __updateScreenLoop(__obj):
    __obj.updateScreenLoop()

class PizRemote:
    pig = pigpio.pi()

    leds_pins = [16, 20, 21, 26, 19, 13, 6, 5]

    safran_channel = 1
    safran_trim_channel = 0
    moteur_channel = 7
    
    safran_trim = -8000

    spi = spidev.SpiDev()
    spi.open(0,0)

    spi.max_speed_hz = 488000 # 15600000 # 7800000 # 488000 # 15600000 #

    pizboat_host = '10.250.1.2'
    pizboat_port = 10012

    sock = socket.socket()

    device = ssd1306(i2c(port=1, address=0x3C))

    class ScreenStats:
        updateScreenThread = None
        linkQuality = 0
        safran_val = 0
        moteur_val = 0

    screenStats = ScreenStats()

    def readChannel(self, channel):
        adc = self.spi.xfer2([1, (8 + channel) << 4, 0])
        data = ((adc[1] & 3) << 8) + adc[2]
        return int(data * 64)

    nbConnects = 0

    def __init__(self):
        with canvas(self.device) as draw:
            draw.rectangle(self.device.bounding_box, outline="white", fill="black")
            draw.text((2, 2), "Init...", fill="white")
        for p in self.leds_pins:
            self.pig.set_mode(p, pigpio.OUTPUT)
        for p in self.leds_pins:
            self.pig.write(p, 1)
            time.sleep(.1)
        time.sleep(.2)
        for p in self.leds_pins:
            self.pig.write(p, 0)
            time.sleep(.1)
        self.screenStats.updateScreenThread = threading.Thread(target=self.updateScreenLoop, args=[])
        self.screenStats.updateScreenThread.start()

    def age(self):
        return int((now() - self.tsBirth) / 1000)

    def connect(self):
        self.sock = socket.socket()
        try:
            startConnect = now()
            addr = socket.getaddrinfo(self.pizboat_host, self.pizboat_port)[0][-1]
            self.sock.settimeout(1000)
            self.sock.connect(addr)
            self.sock.setblocking(False)
            endConnect = now()
            logger.info(f"Time to connect: {endConnect - startConnect}")
            self.nbConnects += 1
        except Exception as err:
            logger.warning(f"Could not connect {err=}, {type(err)=}")
            raise

    nbPackets = 0

    lastUpdate = 0

    lastScreenUpdate = 0

    tsBirth = now()

    isConnected = False

    linkQualityMin = 100
    linkQualityMax = 0


    def run(self):
        while True:
            cnt = 0
            while not self.isConnected:
                for p in range(0, len(self.leds_pins)):
                    self.pig.write(self.leds_pins[p], 1 if p == (cnt % len(self.leds_pins)) else 0)
                logger.info(f"Age {self.age()} Connecting attempt {cnt}...")
                with canvas(self.device) as draw:
                    draw.rectangle(self.device.bounding_box, outline="white", fill="black")
                    draw.text((2, 2), f"Connecting #{cnt}...", fill="white")

                try:
                    self.connect()
                    self.isConnected = True
                    logger.info("Connected !")
                    with canvas(self.device) as draw:
                        draw.rectangle(self.device.bounding_box, outline="white", fill="black")
                        draw.text((2, 2), f"Connected !", fill="white")

                except:
                    time.sleep(1)
                cnt += 1
            self.pig.write(self.leds_pins[0], 1)

            try:
                self.doLoop()
                time.sleep(.04)

            except (ConnectionResetError, BrokenPipeError) as err:
                logger.warning(f"Connection reset ! {err=}")
                self.isConnected = False
            except Exception as err:
                print(f"Unexpected {err=}, {type(err)=}")
                logger.warning("Caught exception %s %s", err, type(err))
                raise

    def doLoop(self):
        safran_trim_raw = self.readChannel(self.safran_trim_channel)

        safran_raw = self.readChannel(self.safran_channel)
        
        safran_val = safran_raw + (safran_trim_raw - (1 << 15))

        moteur_raw = self.readChannel(self.moteur_channel)
        moteur_val = moteur_raw

        tsMessageSent = now()

        self.pig.write(self.leds_pins[0], 0)

        self.sock.send(bytes("{\"safran\":" + str(safran_val) + ", \"moteur\":" + str(moteur_val) + ", \"ts\":" + str(round(tsMessageSent)) + "}\n", 'utf8'))

        self.nbPackets += 1

        ready = select.select([self.sock], [], [], 2)

        if not ready[0]:
            logger.warning("Lost connection !")
            self.isConnected = False
            return

        ack_raw = self.sock.recv(4096)

        if len(ack_raw) == 0:
            logger.warning("Empty ack message !")
            self.isConnected = False
            return

        try:
            ack = json.loads(ack_raw)
        except Exception as err:
            logger.warning(f"Invalid JSON contents {ack_raw=} {type(err)=} {err=}")
            raise

        timeSend = now() - tsMessageSent

        tsMessageRecieved = ack["ts"]
        linkQuality = ack["linkQuality"]

        self.updateLinkQuality(linkQuality)

        self.screenStats.safran_val = safran_val
        self.screenStats.moteur_val = moteur_val
        self.screenStats.linkQuality = linkQuality

        if now() - self.lastUpdate > 1000:
            lag = tsMessageSent - tsMessageRecieved
            messageRate = self.nbPackets / self.age()
            logger.info(f"Uptime={self.age()}, safran={safran_val} (trim={safran_trim_raw}) moteur={moteur_val}, {self.nbConnects=}, {messageRate=}, {lag=}, {timeSend=}, {linkQuality=} min={self.linkQualityMin} max={self.linkQualityMax}")
            self.lastUpdate = now()

    def updateScreenLoop(self):
        while True:
            if self.isConnected:
                screenRefreshStart = now()
                self.updateScreen()
                logger.info(f"Took {now() - screenRefreshStart}ms to refresh screen")
            time.sleep(.5)

    def updateScreen(self):
        linkQuality = self.screenStats.linkQuality
        safran_val = self.screenStats.safran_val
        moteur_val = self.screenStats.moteur_val

        with canvas(self.device) as draw:
            # draw.rectangle(self.device.bounding_box, outline="black", fill="black")
            draw.text((2, 2), f"Wifi " + str(linkQuality), fill="white")
            
            wpos = int(self.device.width * safran_val / 65536)
            draw.rectangle((wpos - 1, self.device.height - 2, wpos + 1, self.device.height), fill="white")
            draw.rectangle((self.device.width - 2, self.device.height - int(self.device.height * moteur_val / 65536), self.device.width, self.device.height), fill="white")

    def updateLinkQuality(self, linkQuality):
        self.linkQualityMin = min(self.linkQualityMin, linkQuality)
        self.linkQualityMax = max(self.linkQualityMax, linkQuality)

        lv = (linkQuality - 30) * 8 / 40
        for p in range(0, len(self.leds_pins)):
            self.pig.write(self.leds_pins[p], 1 if p < lv else 0)


if __name__ == "__main__":
    remote = PizRemote()
    remote.run()
