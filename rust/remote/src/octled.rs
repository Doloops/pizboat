use rppal::gpio::{Gpio, OutputPin};
use std::thread;
use std::time::Duration;

pub struct OctLed {
    pins: Vec<OutputPin>,
}

impl OctLed {
    pub fn new(pin_numbers: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let gpio = Gpio::new()?;
        let mut pins = Vec::new();

        for &pin_num in pin_numbers {
            let pin = gpio.get(pin_num)?.into_output();
            println!("GPIO Led {} initialized", pin_num);
            pins.push(pin);
        }

        Ok(OctLed { pins })
    }
    
    pub fn k2000(&mut self) {
        
        for x in &mut self.pins
        {
            x.set_high();
        }
        thread::sleep(Duration::from_millis(40));
        for x in &mut self.pins
        {
            x.set_low();
        }
        thread::sleep(Duration::from_millis(40));

        for n in 0..11 {
            if n < 8
            {
                self.pins[n].set_high();
            }
            if n >= 3
            {
                self.pins[n - 3].set_low();
            }
            thread::sleep(Duration::from_millis(40));
        }
    }
    
    pub fn display_value(&mut self, uval: u8) {
        let val = uval.clamp(0, 8);
        
        for n in 0..8 {
            if n < val { self.pins[n as usize].set_high(); }
            else { self.pins[n as usize].set_low(); }
        }
    }
}
