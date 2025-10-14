use rust_pigpio::{initialize, set_mode, read, write, INPUT, OUTPUT, ON, OFF};

use std::time::{Duration, SystemTime};
use std::thread;


/// HX711 gain settings which also select the channel
#[derive(Clone, Copy, Debug)]
pub enum Gain {
    /// Channel A with gain of 128 (default)
    ChAGain128 = 1,
    /// Channel B with gain of 32
    ChBGain32 = 2,
    /// Channel A with gain of 64
    ChAGain64 = 3,
}

pub struct HX711 {
    pd_sck_pin: u32,
    dout_pin: u32,
    gain: Gain,
}

impl HX711 {
    /// Create a new HX711 instance
    /// 
    /// # Arguments
    /// * `dout_pin` - GPIO pin number for data output (DOUT)
    /// * `pd_sck_pin` - GPIO pin number for power down and serial clock (PD_SCK)
    /// * `gain` - Initial gain setting (default: ChAGain128)
    pub fn new(dout_pin: u32, pd_sck_pin: u32, gain: Gain) -> Result<Self, Box<dyn std::error::Error>> {
        initialize().expect("Could not init pigpio !");

        set_mode(pd_sck_pin, OUTPUT)?;
        set_mode(dout_pin, INPUT)?;
        
        write(pd_sck_pin, OFF).unwrap();
        // let mut pd_sck = gpio.get(pd_sck_pin)?.into_output();
        // let dout = gpio.get(dout_pin)?.into_input();
        
        // pd_sck.set_low();
        
        let mut hx711 = HX711 {
            pd_sck_pin,
            dout_pin,
            gain,
        };
        
        // Initial setup delay
        thread::sleep(Duration::from_micros(100));
        
        // Reset to ensure proper state
        hx711.reset();
        
        hx711.init();
        
        Ok(hx711)
    }
    
    pub fn init(&self) {
        for _n in 0..10 {
            write(self.pd_sck_pin, ON).unwrap();
            self.do_sleep();
            write(self.pd_sck_pin, OFF).unwrap();
            self.do_sleep();
        }
    }
    
    pub fn do_sleep(&self) {
        let start = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();

        loop {
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
            let diff = now - start;
            // println!("diff {}", diff);
            
            if diff > 5000
            {
                break;
            }
        }
    }

    /// Check if the HX711 is ready to send data
    pub fn is_ready(&self) -> bool {
        // self.dout.is_low()
        let value = read(self.dout_pin).unwrap();
        return value == 0;
    }
    
    /// Read raw 24-bit value from the HX711
    fn read_raw_bytes(&mut self) -> i32 {
        // Wait until HX711 is ready (with a simple timeout)
        
        let mut timeout = 0;
        while !self.is_ready() {
            timeout += 1;
            if timeout > 1000000 {
                return -1; // Timeout after ~1 second
            }
            thread::sleep(Duration::from_micros(1));
        }
        let mut count: i32 = 0;
        
        // Read three bytes of data
        for _bit in 0..24 {
            write(self.pd_sck_pin, ON).unwrap();
            self.do_sleep();

            write(self.pd_sck_pin, OFF).unwrap();
            self.do_sleep();            
            
            // Read bit based on bit format
            // let bit_value = if self.dout.is_high() { 1 } else { 0 };
            let bit_value = read(self.dout_pin).unwrap() as u8;
            
            count <<= 1;
            count += bit_value as i32;
        }

        
        // Set gain for next reading by sending additional clock pulses
        for _ in 0..(self.gain as u8) {
            // self.pd_sck.set_high();
            write(self.pd_sck_pin, ON).unwrap();
            self.do_sleep();
            
            //self.pd_sck.set_low();
            write(self.pd_sck_pin, OFF).unwrap();
            self.do_sleep();
        }
        
        count = count ^ 0x800000;
        count
    }
    
    /// Get a single reading
    pub fn get_value(&mut self) -> Option<i32> {
        let value = self.read_raw_bytes();
        if value == -1 { None } else { Some(value) }
    }
        
    /// Power down the HX711
    pub fn power_down(&mut self) {
        println!("power_down()");
        write(self.pd_sck_pin, OFF).unwrap();
        thread::sleep(Duration::from_micros(100));
        write(self.pd_sck_pin, ON).unwrap();
        
        // Wait 100 microseconds (HX711 powers down after 60us)
        thread::sleep(Duration::from_micros(100));
    }
    
    /// Power up the HX711
    pub fn power_up(&mut self) {
        println!("power_up()");
        // self.pd_sck.set_low();
        write(self.pd_sck_pin, OFF).unwrap();
        
        // Wait 100 microseconds for HX711 to power back up
        thread::sleep(Duration::from_micros(100));
        
        // HX711 defaults to Channel A with gain 128 after power up
        // If we need a different setting, read and discard one value
        // if !matches!(self.gain, Gain::ChAGain128) {
        //            self.read_raw_bytes();
        //}
    }
    
    /// Reset the HX711 (power cycle)
    pub fn reset(&mut self) {
        self.power_down();
        self.power_up();
    }
}

// Example usage
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore] // Ignore by default as it requires actual hardware
    fn test_basic_reading() {
        let mut hx711 = HX711::new(5, 6, Gain::ChAGain128).unwrap();
        
        // Set reading format if needed
        hx711.set_reading_format(ByteFormat::MSB, BitFormat::MSB);
        
        // Set calibration values
        hx711.set_offset_a(8388608);
        hx711.set_reference_unit_a(432.0);
        
        // Read weight
        if let Some(weight) = hx711.get_weight(5) {
            println!("Weight: {:.2} g", weight);
        }
    }
}
