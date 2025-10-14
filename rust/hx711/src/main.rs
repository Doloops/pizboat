use std::thread;
use std::time::Duration;

mod hx711; // Assuming the driver is in hx711.rs
use hx711::{HX711, Gain};

// Your calibration constants
const OFFSET: i32 = 8388608;  // Zero offset value
const SCALE: f32 = 432.0;     // Scale factor (raw units per gram)

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing HX711...");
    
    // Initialize HX711
    // DOUT = GPIO 5, PD_SCK = GPIO 6
    let mut hx711 = HX711::new(5, 6, Gain::ChAGain128)?;

    // hx711.doloop();
    
    // Set calibration values
    hx711.set_offset_a(OFFSET);
    hx711.set_reference_unit_a(SCALE);
    
    println!("HX711 ready!");
    println!("Starting continuous reading...\n");
    
    // Continuous reading loop
    loop {
        match hx711.get_value() {
            Some(raw_value) => {
                // Calculate weight using calibration
                let weight = (raw_value - OFFSET) as f32 / SCALE;
                println!("Raw: {:8} | Weight: {:8.2} g", raw_value, weight);
            }
            None => {
                println!("Error: Failed to read from sensor");
            }
        }
        
        thread::sleep(Duration::from_millis(200));
    }
}
