mod hx711;

use hx711::{HX711, Gain};

use anyhow::Result;
use rust_pigpio::{initialize, pwm::servo};
use serde::{Deserialize, Serialize};
use std::thread;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::{connect, Message};
use std::fs;

const WS_URL: &str = "ws://10.250.1.1:10013";

#[derive(Debug, Serialize)]
struct QueryMessage {
    #[serde(rename = "type")]
    msg_type: String,
    timestamp: u64,
    wireless_quality: i16,
    latency: u64,
    weight: f32
}

#[derive(Debug, Deserialize)]
struct CommandResponse {
    #[serde(rename = "type")]
    msg_type: String,
    timestamp: u64,
    rudder_star: Option<u32>,
    rudder_port: Option<u32>,
    motor: Option<u32>,
    boom: Option<u32>,
    genoa: Option<u32>,
}

struct ServoController {
    name: String,
    pin_number: u32,
}

impl ServoController {
    fn new(name: &str, pin_number: u32) -> Result<Self> {
        let default_pulse_width_us = 1450;
        
        servo(pin_number, default_pulse_width_us)
          .map_err(|e| anyhow::anyhow!("Servo {} error: {}", name, e))?;

        println!("Init servo {} to pin {}", name.to_string(), pin_number);

        Ok(Self { name: name.to_string(), pin_number})
    }

    fn set_servo_pulse(&mut self, pulse_width_us: u32) -> Result<()> {
        let pulse_width_us = pulse_width_us.clamp(1000, 2000);

        servo(self.pin_number, pulse_width_us)
          .map_err(|e| anyhow::anyhow!("Servo {} error: {}", self.name, e))?;

        Ok(())
    }
}

struct BoatController {
    rudder_star: ServoController,
    rudder_port: ServoController,
    motor: ServoController,
    boom: ServoController,
    genoa: ServoController,
}

impl BoatController {
    fn new() -> Result<Self, anyhow::Error> {
        Ok(Self {
            rudder_star: ServoController::new("rudder_star", 23)?,
            rudder_port: ServoController::new("rudder_port", 24)?,
            motor: ServoController::new("motor", 25)?,
            boom: ServoController::new("boom", 22)?,
            genoa: ServoController::new("genoa", 27)?,
        })
    }
    
    fn apply_commands(&mut self, cmd: &CommandResponse) -> Result<()> {
        if let Some(val) = cmd.rudder_star {
            self.rudder_star.set_servo_pulse(val)?;
        }
        if let Some(val) = cmd.rudder_port {
            self.rudder_port.set_servo_pulse(val)?;
        }
        if let Some(val) = cmd.motor {
            self.motor.set_servo_pulse(val)?;
        }
        if let Some(val) = cmd.boom {
            self.boom.set_servo_pulse(val)?;
        }
        if let Some(val) = cmd.genoa {
            self.genoa.set_servo_pulse(val)?;
        }
        Ok(())
    }    
}

fn get_wireless_link_quality() -> i16 {
    match read_link_quality() {
        Ok(quality) => quality,
        Err(err) => {
            // Equivalent of logger.info in Python
            eprintln!("Caught exception: {:?}", err);
            -1
        }
    }
}

fn read_link_quality() -> Result<i16, Box<dyn std::error::Error>> {
    let content = fs::read_to_string("/proc/net/wireless")?;
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 3 { return Ok(-1); }
    let data_line = lines[2].trim();
    let fields: Vec<&str> = data_line.split_whitespace().collect();
    if fields.len() <= 2 { return Ok(-1); }
    let link_quality = fields[2];
    let link_quality = link_quality.trim_end_matches('.');
    let quality = link_quality.parse::<i16>()?;
    Ok(quality)
}

fn get_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn handle_websocket(controller: &mut BoatController, weight_mutex: Arc<Mutex<Option<f32>>>) -> Result<()> {
    let (mut socket, _response) = connect(WS_URL)?;
    println!("WebSocket connected to {}", WS_URL);

    let mut counter = 0;
    let max_counter = 1000 / 40;
    
    let mut latency = 0;
    

    loop {
        let timestamp = get_timestamp_ms();
        let wireless_quality = get_wireless_link_quality();
        
        let weight = match *(weight_mutex.lock().unwrap())
        {
        Some(d) => { d }
        None => { -1 as f32 }
        };
        
        let query = QueryMessage {
            msg_type: "query".to_string(),
            timestamp,
            wireless_quality,
            latency,
            weight
        };
        
        let query_json = serde_json::to_string(&query)?;
        
        // println!("Update {query_json}");
        socket.send(Message::Text(query_json))?;
        
        match socket.read() {
            Ok(Message::Text(text)) => {
                // println!("Update {text}");
                match serde_json::from_str::<CommandResponse>(&text) {
                    Ok(response) => {
                        let now = get_timestamp_ms();
                        latency = now.saturating_sub(response.timestamp);
                        
                        if let Err(e) = controller.apply_commands(&response) {
                            eprintln!("Error applying command: {}", e);
                        } else {
                            // println!("Commands applied - lag: {}ms", lag_ms);
                            counter += 1;
                            if counter % max_counter == 0
                            {
                                println!("Counter {} wireless quality: {} lag: {}ms", counter, wireless_quality, latency);
                            }
                        }
                    }
                    Err(e) => eprintln!("JSON parse error: {}", e),
                }
            }
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
        
        thread::sleep(Duration::from_millis(40));
    }

    Ok(())
}

// Your calibration constants
const OFFSET: i32 = 8661777;  // Zero offset value
const SCALE: f32 = 960.33;     // Scale factor (raw units per gram)


fn hx711_thread(weight_mutex: Arc<Mutex<Option<f32>>>) {
    let mut hx711 = HX711::new(5, 6, Gain::ChAGain128).expect("Could not init hx711 :");

    loop {
         match hx711.get_value() {
            Some(raw_value) => {
                // Calculate weight using calibration
                let weight = (raw_value - OFFSET) as f32 / SCALE;
                // println!("Raw: {:8} | Weight: {:8.2} g", raw_value, weight);
                
                {
                    let mut locked = weight_mutex.lock().unwrap();
                    *locked = Some(weight);
                }
            }
            None => {
                println!("Error: Failed to read from sensor");
                {
                    let mut locked = weight_mutex.lock().unwrap();
                    *locked = None;
                }

            }
        }
        thread::sleep(Duration::from_micros(20_000));
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize().expect("Could not init pigpio !");
    

    let weight_mutex: Arc<Mutex<Option<f32>>> = Arc::new(Mutex::new(None));
    let weight_mutex_clone = Arc::clone(&weight_mutex);
    
    
    thread::spawn(move || hx711_thread(weight_mutex_clone));

    let mut controller = BoatController::new()?;

    loop {
        println!("Connecting to {}", WS_URL);
        if let Err(e) = handle_websocket(&mut controller, Arc::clone(&weight_mutex)) {
            eprintln!("Connection error: {}", e);
            thread::sleep(Duration::from_secs(1));
        }
    }
}
