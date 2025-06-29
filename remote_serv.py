import logging
import spidev
import time
import socket
import select
import json
import pigpio

logger = logging.getLogger(__name__)
logging.basicConfig(filename="/home/pi/remote.log", encoding="utf-8", level=logging.DEBUG)

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

class PizRemote:
    pig = pigpio.pi()

    leds_pins = [5, 6, 13, 19, 26]

    spi = spidev.SpiDev()
    spi.open(0,0)

    spi.max_speed_hz = 488000 # 15600000 # 7800000 # 488000 # 15600000 #

    pizboat_host = '10.250.1.2'
    pizboat_port = 10012

    sock = socket.socket()

    def readChannel(self, channel):
        adc = self.spi.xfer2([1, (8 + channel) << 4, 0])
        data = ((adc[1] & 3) << 8) + adc[2]
        return int(data * 64)

    nbConnects = 0

    def __init__(self):
        for p in self.leds_pins:
            self.pig.set_mode(p, pigpio.OUTPUT)
        for p in self.leds_pins:
            self.pig.write(p, 1)
            time.sleep(.1)
        time.sleep(.2)
        for p in self.leds_pins:
            self.pig.write(p, 0)
            time.sleep(.1)

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

    tsBirth = now()

    isConnected = False

    linkQualityMin = 100
    linkQualityMax = 0


    def run(self):

        while True:
            cnt = 0
            while not self.isConnected:
                for p in range(0, len(self.leds_pins)):
                    self.pig.write(self.leds_pins[p], 1 if p == (cnt % 5) else 0)
                logger.info(f"Age {self.age()} Connecting attempt {cnt}...")
                try:
                    self.connect()
                    self.isConnected = True
                    logger.info("Connected !")
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

    safran = CircBuffer(5)
    moteur = CircBuffer(3)

    def doLoop(self):
        self.safran.add(self.readChannel(0))
        self.moteur.add(self.readChannel(1))

        tsMessageSent = now()

        self.pig.write(self.leds_pins[0], 0)

        self.sock.send(bytes("{\"safran\":" + str(self.safran.average()) + ", \"moteur\":" + str(self.moteur.average()) + ", \"ts\":" + str(round(tsMessageSent)) + "}\n", 'utf8'))

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

        if now() - self.lastUpdate > 1000:
            lag = tsMessageSent - tsMessageRecieved
            messageRate = self.nbPackets / self.age()
            logger.info(f"Uptime={self.age()}, safran={self.safran.average()} moteur={self.moteur.average()}, {self.nbConnects=}, {messageRate=}, {lag=}, {timeSend=}, {linkQuality=} min={self.linkQualityMin} max={self.linkQualityMax}")
            self.lastUpdate = now()

    def updateLinkQuality(self, linkQuality):
        self.linkQualityMin = min(self.linkQualityMin, linkQuality)
        self.linkQualityMax = max(self.linkQualityMax, linkQuality)

        lv = (linkQuality - 30) * 5 / 40
        for p in range(0, len(self.leds_pins)):
            self.pig.write(self.leds_pins[p], 1 if p < lv else 0)


if __name__ == "__main__":
    remote = PizRemote()
    remote.run()
