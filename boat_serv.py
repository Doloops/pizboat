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

class PizBoat:

	pi = pigpio.pi()
	safran_pins=[23, 24]
	safran_min=1200
	safran_max=1800
	safran_mid=int((safran_min+safran_max)/2)

	moteur_pin=25
	moteur_min=1100
	moteur_max=1900
	moteur_mid=1400 #int((moteur_min+moteur_max)/2)

	listen_address = '0.0.0.0'
	listen_port = 10012	
	sock = socket.socket()	

	moteur_avg = CircBuffer()

	def __init__(self):
		for p in self.safran_pins:
			self.pi.set_mode(p, pigpio.OUTPUT)
			self.pi.set_servo_pulsewidth(p, self.safran_mid)

		self.pi.set_mode(self.moteur_pin, pigpio.OUTPUT)
		self.pi.set_servo_pulsewidth(self.moteur_pin, self.moteur_min)

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
			self.pi.set_servo_pulsewidth(self.moteur_pin, self.moteur_mid)
			logger.info("Waiting connection..")
			client, addr = self.sock.accept()
			logger.info(f"client connected from {client}, {addr}")
			client.setblocking(0)
			self.handleClient(client, addr)
				
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

				safran_raw = json_data["safran"]
				safran_val = int(self.safran_min + ((safran_raw / 65536) * (self.safran_max - self.safran_min)))

				moteur_raw = json_data["moteur"]
				moteur_val = int(self.moteur_min + ((moteur_raw / 65536) * (self.moteur_max - self.moteur_min)))
                
				self.moteur_avg.add(moteur_val)
				moteur_val = self.moteur_avg.average()

				ts=json_data["ts"]

				# logger.info("safran_raw=" + str(safran_raw) + ", safran_val=" + str(safran_val) + ", moteur_raw=" + str(moteur_raw) + ", moteur_val=" + str(moteur_val))
				# logger.info(f"{safran_raw=} {safran_val=} {moteur_raw=} {moteur_val=}")

				for p in self.safran_pins:
					self.pi.set_servo_pulsewidth(p, safran_val)
				self.pi.set_servo_pulsewidth(self.moteur_pin, moteur_val) 

				client.send(bytes("{\"status\":\"ok\",\"ts\":" + str(ts) + ",\"myTs\":" + str(round(time.time() * 1000)) + ",\"linkQuality\":" + str(self.get_wireless_link_quality()) + "}", 'utf8'))

			except Exception as err:
				logger.info(f"Unexpected {err=}, {type(err)=}")
				client.close()
				raise
				break


if __name__ == "__main__":
    boat = PizBoat()
    boat.run()
