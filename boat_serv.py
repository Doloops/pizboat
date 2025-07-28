import logging
import socket
import json
import select
import pigpio
import time

logger = logging.getLogger(__name__)
logging.basicConfig(filename="/home/pi/boat.log", encoding="utf-8", level=logging.DEBUG)

logger.info("***************************")
logger.info("* Starting PIZ Boat Now ! *")
logger.info("***************************")
logger.info("")

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

class Actuator:
    def __init__(self, name, pin, value_mid, value_range, dead_range):
        self.name = name
        self.pin = pin
        self.value_mid = value_mid
        self.value_range = value_range
        self.dead_range = dead_range
        self.value_min = value_mid - (value_range / 2)
        self.value_max = value_mid + (value_range / 2)
        self.value = self.value_mid
        
    def updateValue(self, json):
        raw = json[self.name] / 65536
        self.value = int(self.value_min + (raw * self.value_range))
        if abs(self.value - self.value_mid) < self.dead_range:
            self.value = self.value_mid

class PizBoat:

    pi = pigpio.pi()
    
    safran = Actuator("safran", [23, 24], 1450, 600, 30)
    moteur = Actuator("moteur", 25, 1400, 800, 50)

    ecoute_gv = Actuator("ecoute_gv", 22, 1450, 1000, 50)
    ecoute_foc = Actuator("ecoute_foc", 27, 1450, 1000, 50)
    
    listen_address = '0.0.0.0'
    listen_port = 10012
    sock = socket.socket()

    moteur_avg = CircBuffer()

    def __init__(self):
        for p in self.safran.pin:
            self.pi.set_mode(p, pigpio.OUTPUT)
            self.pi.set_servo_pulsewidth(p, self.safran.value_mid)

        for actuator in [self.moteur, self.ecoute_gv, self.ecoute_foc]:
            self.pi.set_mode(actuator.pin, pigpio.OUTPUT)
            self.pi.set_servo_pulsewidth(actuator.pin, actuator.value_mid)

        addr = socket.getaddrinfo(self.listen_address, self.listen_port)[0][-1]

        self.sock.bind(addr)
        self.sock.listen(1)

    def get_wireless_link_quality(self):
        try:
            with open('/proc/net/wireless', 'r') as f:
                lines = f.readlines()

            if len(lines) < 3:
                return -1
            data_line = lines[2].strip()
            fields = data_line.split()

            # Line structure: interface_name: status link level noise nwid crypt frag retry misc beacon
            link_quality = fields[2]

            # logger.info(f"Link Quality: {link_quality}")
            return int(link_quality.rstrip('.'))
        except Exception as err:
            logger.info(f"Caught exception {err=} {type(err)=}")
            return -1

    def run(self):
        while True:
            # Reinit boat engine
            self.pi.set_servo_pulsewidth(self.moteur.pin, self.moteur.value_mid)
            logger.info("Waiting connection..")
            client, addr = self.sock.accept()
            logger.info(f"client connected from {client}, {addr}")
            client.setblocking(0)
            self.handleClient(client, addr)
            
    def updatePwm(self, actuator):
        pin = actuator.pin
        val = actuator.value
        if type(pin) in (tuple, list):
            for p in pin:
                self.pi.set_servo_pulsewidth(p, val)
        else:
            self.pi.set_servo_pulsewidth(pin, val)

    def handleClient(self, client, addr):
        while True:
            try:
                ready = select.select([client], [], [], 1)
                if not ready[0]:
                    logger.info("Not ready !")
                    break
                line = client.recv(4096)
                if len(line) == 0:
                    logger.info("Empty buffer !")
                    break
                json_data = json.loads(line.decode('utf8'))

                for actuator in [self.safran, self.moteur, self.ecoute_gv, self.ecoute_foc]:
                    actuator.updateValue(json_data)
                    self.updatePwm(actuator)

                ts=json_data["ts"]

                logger.info(f"safran={self.safran.value} moteur={self.moteur.value} ecoute_gv={self.ecoute_gv.value} ecoute_foc={self.ecoute_foc.value}")

                client.send(bytes("{\"status\":\"ok\",\"ts\":" + str(ts) + ",\"myTs\":" + str(round(time.time() * 1000)) + ",\"linkQuality\":" + str(self.get_wireless_link_quality()) + "}", 'utf8'))

            except Exception as err:
                logger.info(f"Unexpected {err=}, {type(err)=}")
                client.close()
                raise
                break


if __name__ == "__main__":
    boat = PizBoat()
    boat.run()
