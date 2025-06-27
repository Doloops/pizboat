# pizboat
Boat and Remote control managed with two tiny shiny PI Zero W2

## At boat side

The servos and motor are controlled via PWM, with the pigpio lib.


## At remote side

The remote controller serves as a WiFi access point.

It uses a MCP3008 to convert the slide potentiometer values to digital.

## Protocol

The remote sends a JSON fragment to the boat
{'safran':value, 'moteur': value', 'ts': timestamp }

The boat replies with a JSON fragment
{'status':'ok', 'ts': the timestamp sent, 'linkQuality': wifi quality}
