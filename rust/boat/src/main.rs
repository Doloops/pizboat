use anyhow::Result;
use rppal::gpio::{Gpio, OutputPin};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tungstenite::{connect, Message};

const WS_URL: &str = "ws://10.250.1.1:10013";
const PWM_FREQUENCY: f64 = 50.0;

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
    rudder_star: Option<u16>,
    rudder_port: Option<u16>,
    motor: Option<u16>,
}

struct ServoController {
    pins: HashMap<String, OutputPin>,
}

impl ServoController {
    fn new() -> Result<Self> {
        let gpio = Gpio::new()?;
        let mut pins = HashMap::new();
        
        let mut rudder_pin_23 = gpio.get(23)?.into_output();
        rudder_pin_23.set_pwm_frequency(PWM_FREQUENCY, 0.0)?;
        pins.insert("rudder_star".to_string(), rudder_pin_23);
        
        let mut rudder_pin_24 = gpio.get(24)?.into_output();
        rudder_pin_24.set_pwm_frequency(PWM_FREQUENCY, 0.0)?;
        pins.insert("rudder_port".to_string(), rudder_pin_24);
        
        let mut motor_pin = gpio.get(25)?.into_output();
        motor_pin.set_pwm_frequency(PWM_FREQUENCY, 0.0)?;
        pins.insert("motor".to_string(), motor_pin);

        Ok(Self { pins })
    }

    fn set_servo_pulse(&mut self, servo_name: &str, pulse_width_us: u16) -> Result<()> {
        let pulse_width_us = pulse_width_us.clamp(1000, 2000);
        let period_us = 1_000_000.0 / PWM_FREQUENCY;
        let duty_cycle = (pulse_width_us as f64) / period_us;

        if let Some(pin) = self.pins.get_mut(servo_name) {
            println!("pin duty_cycle {}", duty_cycle);
            pin.set_pwm_frequency(PWM_FREQUENCY, duty_cycle)?;
        }
        else {
            println!("Unknown servo {}", servo_name);
        }

        Ok(())
    }

    fn apply_commands(&mut self, cmd: &CommandResponse) -> Result<()> {
        if let Some(val) = cmd.rudder_star {
            self.set_servo_pulse("rudder_star", val)?;
        }
        if let Some(val) = cmd.rudder_port {
            self.set_servo_pulse("rudder_port", val)?;
        }
        if let Some(val) = cmd.motor {
            self.set_servo_pulse("motor", val)?;
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

fn handle_websocket(controller: &mut ServoController) -> Result<()> {
    let (mut socket, _response) = connect(WS_URL)?;
    println!("WebSocket connected to {}", WS_URL);

    let query_interval = Duration::from_millis(20);
    
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
        
        thread::sleep(query_interval);
    }

    Ok(())
}

fn main() -> Result<()> {
    let mut controller = ServoController::new()?;

    loop {
        println!("Connecting to {}", WS_URL);
        if let Err(e) = handle_websocket(&mut controller) {
            eprintln!("Connection error: {}", e);
            thread::sleep(Duration::from_secs(5));
        }
    }
}
