use anyhow::Result;
use rust_pigpio::pigpio;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::{connect, Message};

const WS_URL: &str = "ws://10.250.1.1:10013";

#[derive(Debug, Serialize)]
struct QueryMessage {
    #[serde(rename = "type")]
    msg_type: String,
    timestamp: u64,
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
        let default_pulse_width_us = 1480;
        
        pigpio::servo(pin_number, default_pulse_width_us)
          .map_err(|e| anyhow::anyhow!("Servo {} error: {}", name, e))?;

        Ok(Self { name: name.to_string(), pin_number})
    }

    fn set_servo_pulse(&mut self, pulse_width_us: u32) -> Result<()> {
        let pulse_width_us = pulse_width_us.clamp(1000, 2000);

        pigpio::servo(self.pin_number, pulse_width_us)
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
            rudder_port: ServoController::new("rudder_star", 24)?,
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

fn get_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn handle_websocket(controller: &mut BoatController) -> Result<()> {
    let (mut socket, _response) = connect(WS_URL)?;
    println!("WebSocket connected to {}", WS_URL);

    loop {
        let timestamp = get_timestamp_ms();
        
        let query = QueryMessage {
            msg_type: "query".to_string(),
            timestamp,
        };
        
        let query_json = serde_json::to_string(&query)?;
        socket.send(Message::Text(query_json))?;
        
        match socket.read() {
            Ok(Message::Text(text)) => {
                println!("Update {text}");
                match serde_json::from_str::<CommandResponse>(&text) {
                    Ok(response) => {
                        let now = get_timestamp_ms();
                        let lag_ms = now.saturating_sub(response.timestamp);
                        
                        if let Err(e) = controller.apply_commands(&response) {
                            eprintln!("Error applying command: {}", e);
                        } else {
                            // println!("Commands applied - lag: {}ms", lag_ms);
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
    }

    Ok(())
}

fn main() -> Result<()> {
    pigpio::initialize()
      .expect("Could not init pigpio !");

    let mut controller = BoatController::new()?;

    loop {
        println!("Connecting to {}", WS_URL);
        if let Err(e) = handle_websocket(&mut controller) {
            eprintln!("Connection error: {}", e);
            thread::sleep(Duration::from_secs(5));
        }
    }
}
