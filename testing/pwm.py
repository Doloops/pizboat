#! /usr/bin/env python3
"""
"""
import sys, os, time
import Pi.GPIO as GPIO

PWM0=12
PWM1=13

# Pi3 & earlier have 19.2MHz clock
# DIVIDER=15  # gives a precise 10kHz signal
# RANGE=128

# Pi4 has 54MHz clock
# DIVIDER=36 # gives a precise 10kHz signal
# RANGE=150
DIVIDER=45 # gives a precise 10kHz signal
RANGE=120

def main():
    GPIO.setmode(GPIO.BCM)

    GPIO.pwm_setmode(0)
    GPIO.pwm_setClock(DIVIDER)

    GPIO.pwm_setGpio(PWM0)
    GPIO.pwm_setGpio(PWM1)

    GPIO.pwm_setmode(GPIO.PWM_MODE_MS)

    GPIO.pwm_setRange(PWM1, RANGE)
    GPIO.pwm_Write(PWM1, RANGE//4)   # duty cycle of 25%

    GPIO.pwm_setRange(PWM0, RANGE)
    GPIO.pwm_Write(PWM0, RANGE//2)   # duty cycle of 50%


if __name__ == '__main__':
    main()
