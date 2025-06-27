import socket
import json
import select
import pigpio
import time

pi = pigpio.pi()
print("Initing...")
safran_pins=[23, 24]
safran_min=1200
safran_max=1800
safran_mid=int((safran_min+safran_max)/2)

moteur_pin=25
moteur_min=1000
moteur_max=2200

for p in safran_pins:
    pi.set_mode(p, pigpio.OUTPUT)
    pi.set_servo_pulsewidth(p, safran_mid)

pi.set_mode(moteur_pin, pigpio.OUTPUT)
pi.set_servo_pulsewidth(moteur_pin, moteur_min)

pizb_port = 10012
addr = socket.getaddrinfo('0.0.0.0', pizb_port)[0][-1]

s = socket.socket()
s.bind(addr)
s.listen(1)

def get_wireless_link_quality():
    try:
        with open('/proc/net/wireless', 'r') as f:
            lines = f.readlines()
                                                                                            
        if len(lines) < 3:
            return -1 
        data_line = lines[2].strip()
        fields = data_line.split()
                                                                                                                                            
        # Line structure: interface_name: status link level noise nwid crypt frag retry misc beacon
        link_quality = fields[2]

        # print(f"Link Quality: {link_quality}")
        return int(link_quality.rstrip('.')) 
    except Exception as err:
        print(f"Caught exception {err=} {type(err)=}")
        return -1 

while True:
    print("Waiting connection..")
    pi.set_servo_pulsewidth(moteur_pin, moteur_min)
    cl, addr = s.accept()
    print('client connected from', addr)
#    cl_file = cl.makefile('rwb', 0)
    cl.setblocking(0)
    while True:
        try:
            ready = select.select([cl], [], [], .5)
            if not ready[0]:
                print("Not ready !")
                break    	
            line = cl.recv(4096)
            if len(line) == 0:
                print("Empty buffer !") 
                break
            json_data = json.loads(line.decode('utf8'))

            safran_raw=json_data["safran"]
            safran_val = int(safran_min + ((safran_raw / 65536) * (safran_max - safran_min)))

            moteur_raw=json_data["moteur"]
            moteur_val = int(moteur_min + ((moteur_raw / 65536) * (moteur_max - moteur_min)))

            ts=json_data["ts"]

            # print("safran_raw=" + str(safran_raw) + ", safran_val=" + str(safran_val) + ", moteur_raw=" + str(moteur_raw) + ", moteur_val=" + str(moteur_val))

            for p in safran_pins:
                pi.set_servo_pulsewidth(p, safran_val)
            pi.set_servo_pulsewidth(moteur_pin, moteur_val) 

            cl.send(bytes("{\"status\":\"ok\",\"ts\":" + str(ts) + ",\"myTs\":" + str(round(time.time() * 1000)) + ",\"linkQuality\":" + str(get_wireless_link_quality()) + "}", 'utf8'))

        except Exception as err:
            print(f"Unexpected {err=}, {type(err)=}")

