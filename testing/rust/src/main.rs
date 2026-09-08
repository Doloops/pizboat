extern crate rust_pigpio;

use std::thread::sleep;
use std::time::Duration;
use rust_pigpio::*;
use rust_pigpio::pwm::*;

const PIN: u32 = 12;

fn main() {
    println!("Initialized pigpio. Version: {}", initialize().unwrap());
    set_mode(PIN, OUTPUT).unwrap();
 
    set_pwm_frequency(PIN, 500).unwrap(); // Set to modulate at 500hz.
    set_pwm_range(PIN, 1000).unwrap(); // Set range to 1000. 1 range = 2 us;
    
    pwm(PIN, 300).unwrap();
    
    sleep(Duration::from_secs(60));
}
