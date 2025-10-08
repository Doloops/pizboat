mod display;
mod pzconfig;

use pzconfig::ChannelConfig;
use display::{DisplayData, display_thread};

use rppal::gpio::{Gpio, InputPin, Level};
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::{Arc, Mutex};
use std::net::TcpListener;
use tungstenite::{accept, Message};

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15];
const DEBOUNCE_MS: u64 = 50;
const LONG_PRESS_MS: u64 = 1000; // 1 second for long press
const ADC_CHANNELS: usize = 8;
const DISPLAY_CHANNELS: [usize; 5] = [0, 1, 2, 6, 7];

#[derive(Debug, Clone, Copy, PartialEq)]
enum ControlMode {
    Normal,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SettingsParameter {
    RudderDeadzone,
    RudderMin,
    RudderMax,
    RudderCenter,
    MotorDeadzone,
    MotorMin,
    MotorMax,
    MotorCenter,
}

impl SettingsParameter {
    fn next(&self) -> Self {
        match self {
            Self::RudderDeadzone => Self::RudderMin,
            Self::RudderMin => Self::RudderMax,
            Self::RudderMax => Self::RudderCenter,
            Self::RudderCenter => Self::MotorDeadzone,
            Self::MotorDeadzone => Self::MotorMin,
            Self::MotorMin => Self::MotorMax,
            Self::MotorMax => Self::MotorCenter,
            Self::MotorCenter => Self::RudderDeadzone,
        }
    }

    fn prev(&self) -> Self {
        match self {
            Self::RudderDeadzone => Self::MotorCenter,
            Self::RudderMin => Self::RudderDeadzone,
            Self::RudderMax => Self::RudderMin,
            Self::RudderCenter => Self::RudderMax,
            Self::MotorDeadzone => Self::RudderCenter,
            Self::MotorMin => Self::MotorDeadzone,
            Self::MotorMax => Self::MotorMin,
            Self::MotorCenter => Self::MotorMax,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::RudderDeadzone => "RUD DZ",
            Self::RudderMin => "RUD MIN",
            Self::RudderMax => "RUD MAX",
            Self::RudderCenter => "RUD CTR",
            Self::MotorDeadzone => "MOT DZ",
            Self::MotorMin => "MOT MIN",
            Self::MotorMax => "MOT MAX",
            Self::MotorCenter => "MOT CTR",
        }
    }

    fn get_value(&self, rudder_cfg: &ChannelConfig, motor_cfg: &ChannelConfig) -> u16 {
        match self {
            Self::RudderDeadzone => rudder_cfg.deadzone,
            Self::RudderMin => rudder_cfg.min,
            Self::RudderMax => rudder_cfg.max,
            Self::RudderCenter => rudder_cfg.center,
            Self::MotorDeadzone => motor_cfg.deadzone,
            Self::MotorMin => motor_cfg.min,
            Self::MotorMax => motor_cfg.max,
            Self::MotorCenter => motor_cfg.center,
        }
    }

    fn set_value(&self, rudder_cfg: &mut ChannelConfig, motor_cfg: &mut ChannelConfig, value: u16) {
        match self {
            Self::RudderDeadzone => rudder_cfg.deadzone = value,
            Self::RudderMin => rudder_cfg.min = value,
            Self::RudderMax => rudder_cfg.max = value,
            Self::RudderCenter => rudder_cfg.center = value,
            Self::MotorDeadzone => motor_cfg.deadzone = value,
            Self::MotorMin => motor_cfg.min = value,
            Self::MotorMax => motor_cfg.max = value,
            Self::MotorCenter => motor_cfg.center = value,
        }
    }
}



#[derive(Debug, Clone, Copy, PartialEq)]
enum Edge {
    Rising,
    Falling,
}

struct ButtonState {
    current: Level,
    last_stable: Level,
    last_change: Instant,
    press_start: Option<Instant>,
}

impl ButtonState {
    fn new() -> Self {
        ButtonState {
            current: Level::Low,
            last_stable: Level::Low,
            last_change: Instant::now(),
            press_start: None,
        }
    }

    fn update(&mut self, new_level: Level) -> Option<Edge> {
        if new_level != self.current {
            self.current = new_level;
            self.last_change = Instant::now();
            return None;
        }

        if self.last_change.elapsed() >= Duration::from_millis(DEBOUNCE_MS)
            && self.current != self.last_stable
        {
            let edge = if self.current == Level::High {
                self.press_start = Some(Instant::now());
                Some(Edge::Rising)
            } else {
                self.press_start = None;
                Some(Edge::Falling)
            };
            self.last_stable = self.current;
            return edge;
        }

        None
    }

    fn is_long_press(&self) -> bool {
        if let Some(start) = self.press_start {
            if self.last_stable == Level::High {
                return start.elapsed() >= Duration::from_millis(LONG_PRESS_MS);
            }
        }
        false
    }
}

struct AdcReader {
    spi: Spi,
}

impl AdcReader {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, 1_000_000, Mode::Mode0)?;
        println!("MCP3008 ADC initialized on SPI0.0");
        Ok(AdcReader { spi })
    }

    fn read_channel(&mut self, channel: u8) -> Result<u16, Box<dyn std::error::Error>> {
        if channel >= 8 {
            return Err("Channel must be 0-7".into());
        }

        let tx_buffer = [
            0x01,
            (0x08 | channel) << 4,
            0x00,
        ];
        let mut rx_buffer = [0u8; 3];

        self.spi.transfer(&mut rx_buffer, &tx_buffer)?;
        
        let buffer = rx_buffer;
        let value = (((buffer[1] & 0x03) as u16) << 8) | (buffer[2] as u16);
        Ok(value)
    }

    fn read_all_channels(&mut self) -> Result<[u16; ADC_CHANNELS], Box<dyn std::error::Error>> {
        let mut values = [0u16; ADC_CHANNELS];
        for channel in 0..ADC_CHANNELS {
            values[channel] = self.read_channel(channel as u8)?;
        }
        Ok(values)
    }
}

struct ButtonReader {
    pins: Vec<InputPin>,
    states: Vec<ButtonState>,
}

impl ButtonReader {
    fn new(pin_numbers: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let gpio = Gpio::new()?;
        let mut pins = Vec::new();
        let mut states = Vec::new();

        for &pin_num in pin_numbers {
            let pin = gpio.get(pin_num)?.into_input_pulldown();
            println!("GPIO {} initialized", pin_num);
            pins.push(pin);
            states.push(ButtonState::new());
        }

        Ok(ButtonReader { pins, states })
    }

    fn read_and_detect_edges(&mut self) -> Vec<Option<Edge>> {
        self.pins
            .iter()
            .enumerate()
            .map(|(i, pin)| {
                let level = pin.read();
                self.states[i].update(level)
            })
            .collect()
    }

    fn get_current_states(&self) -> Vec<Level> {
        self.states.iter().map(|s| s.last_stable).collect()
    }

    fn is_button_long_press(&self, button_index: usize) -> bool {
        if button_index < self.states.len() {
            self.states[button_index].is_long_press()
        } else {
            false
        }
    }
}


fn websocket_thread(data_mutex: Arc<Mutex<Option<DisplayData>>>) {
    let server = TcpListener::bind("0.0.0.0:10013").expect("Failed to bind WebSocket server");
    println!("WebSocket server listening on port 10013");

    for stream in server.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Connection error: {}", e);
                continue;
            }
        };

        let data_mutex = Arc::clone(&data_mutex);
        thread::spawn(move || {
            let mut websocket = match accept(stream) {
                Ok(ws) => ws,
                Err(e) => {
                    eprintln!("WebSocket handshake error: {}", e);
                    return;
                }
            };

            println!("New WebSocket client connected");

            loop {
                let data = {
                    let locked_data = data_mutex.lock().unwrap();
                    locked_data.clone()
                };

                if let Some(d) = data {
                    match serde_json::to_string(&d) {
                        Ok(json) => {
                            if websocket.send(Message::Text(json)).is_err() {
                                println!("WebSocket client disconnected");
                                break;
                            }
                        }
                        Err(e) => eprintln!("JSON serialization error: {}", e),
                    }
                }

                thread::sleep(Duration::from_millis(40));
            }
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting RC Boat Controller with WebSocket");

    let mut button_reader = ButtonReader::new(&BUTTON_PINS)?;
    let mut adc_reader = AdcReader::new()?;

    let (tx_display, rx_display): (SyncSender<DisplayData>, Receiver<DisplayData>) = mpsc::sync_channel(1);
    let data_mutex: Arc<Mutex<Option<DisplayData>>> = Arc::new(Mutex::new(None));
    
    thread::spawn(move || {
        display_thread(rx_display);
    });

    let data_mutex_clone = Arc::clone(&data_mutex);
    thread::spawn(move || {
        websocket_thread(data_mutex_clone);
    });

    let mut last_event = String::from("--");
    let mut mode = ControlMode::Normal;
    let mut rudder_config = ChannelConfig::default();
    let mut motor_config = ChannelConfig::default();
    
    // Settings mode temporary variables
    let mut temp_rudder_config = rudder_config;
    let mut temp_motor_config = motor_config;
    let mut selected_parameter = SettingsParameter::RudderDeadzone;
    let mut mode_just_changed = false;

    loop {
        let edges = button_reader.read_and_detect_edges();
        let button_states = button_reader.get_current_states();
        
        // Check for long press on B3 to toggle mode
        if button_reader.is_button_long_press(3) && !mode_just_changed {
            mode = match mode {
                ControlMode::Normal => {
                    println!("[MODE] Entering Settings mode");
                    temp_rudder_config = rudder_config;
                    temp_motor_config = motor_config;
                    selected_parameter = SettingsParameter::RudderDeadzone;
                    ControlMode::Settings
                }
                ControlMode::Settings => {
                    println!("[MODE] Cancelled Settings, returning to Normal mode");
                    ControlMode::Normal
                }
            };
            last_event = String::from("MODE");
            mode_just_changed = true;
        } else if !button_reader.is_button_long_press(3) {
            mode_just_changed = false;
        }
        
        // Handle button events based on mode
        for (i, &edge) in edges.iter().enumerate() {
            if let Some(Edge::Rising) = edge {
                match mode {
                    ControlMode::Normal => {
                        if i != 3 || !button_reader.is_button_long_press(3) {
                            last_event = format!("B{} PR", i);
                            println!("[EVENT] Button {} pressed (GPIO {})", i, BUTTON_PINS[i]);
                        }
                    }
                    ControlMode::Settings => {
                        match i {
                            3 => {
                                // B3: Next parameter (if not long press)
                                if !button_reader.is_button_long_press(3) {
                                    selected_parameter = selected_parameter.next();
                                    last_event = format!("NEXT");
                                    println!("[SETTINGS] Next parameter: {}", selected_parameter.name());
                                }
                            }
                            5 => {
                                // B5: Previous parameter
                                selected_parameter = selected_parameter.prev();
                                last_event = format!("PREV");
                                println!("[SETTINGS] Previous parameter: {}", selected_parameter.name());
                            }
                            1 => {
                                // B1: Increase value
                                let current = selected_parameter.get_value(&temp_rudder_config, &temp_motor_config);
                                let new_value = (current + 10).min(3000);
                                selected_parameter.set_value(&mut temp_rudder_config, &mut temp_motor_config, new_value);
                                last_event = format!("INC");
                                println!("[SETTINGS] {} = {}", selected_parameter.name(), new_value);
                            }
                            4 => {
                                // B4: Decrease value
                                let current = selected_parameter.get_value(&temp_rudder_config, &temp_motor_config);
                                let new_value = current.saturating_sub(10);
                                selected_parameter.set_value(&mut temp_rudder_config, &mut temp_motor_config, new_value);
                                last_event = format!("DEC");
                                println!("[SETTINGS] {} = {}", selected_parameter.name(), new_value);
                            }
                            0 => {
                                // B0: Cancel
                                mode = ControlMode::Normal;
                                last_event = format!("CANCEL");
                                println!("[SETTINGS] Cancelled, discarding changes");
                            }
                            2 => {
                                // B2: Validate
                                rudder_config = temp_rudder_config;
                                motor_config = temp_motor_config;
                                mode = ControlMode::Normal;
                                last_event = format!("SAVE");
                                println!("[SETTINGS] Settings saved");
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let adc_values = adc_reader.read_all_channels()?;

        // Transform ADC values (rudder on channel 0, motor on channel 1)
        let rudder_value = rudder_config.transform_adc(adc_values[0]);
        let motor_value = motor_config.transform_adc(adc_values[1]);

        let button_states_bool: [bool; 6] = [
            button_states[0] == Level::High,
            button_states[1] == Level::High,
            button_states[2] == Level::High,
            button_states[3] == Level::High,
            button_states[4] == Level::High,
            button_states[5] == Level::High,
        ];

        // In normal mode, send B0, B1, B3, B4 (skip B2 and B5)
        let buttons_sent = [
            button_states_bool[0],
            button_states_bool[1],
            button_states_bool[3],
            button_states_bool[4],
        ];

        let (mode_str, current_param, current_val) = match mode {
            ControlMode::Normal => ("NORMAL", None, None),
            ControlMode::Settings => {
                let param_name = selected_parameter.name().to_string();
                let param_value = selected_parameter.get_value(&temp_rudder_config, &temp_motor_config);
                ("SETTINGS", Some(param_name), Some(param_value))
            }
        };

        let display_data = DisplayData {
            adc_values,
            button_states: button_states_bool,
            buttons_sent,
            rudder_value,
            motor_value,
            mode: mode_str.to_string(),
            rudder_config,
            motor_config,
            current_parameter: current_param,
            current_value: current_val,
            last_event: last_event.clone(),
        };

        let _ = tx_display.try_send(display_data.clone());
        
        {
            let mut locked_data = data_mutex.lock().unwrap();
            *locked_data = Some(display_data);
        }
        
        thread::sleep(Duration::from_millis(40));
    }
}
