mod display;
mod pzconfig;
mod adc;
mod buttons;
mod websocket;

use websocket::websocket_thread;
use pzconfig::{ChannelConfig, SettingsParameter};
use display::{DisplayData, display_thread};
use adc::AdcReader;
use buttons::{ButtonReader, Edge};

use rppal::gpio::{Level};
use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15];
const DEBOUNCE_MS: u64 = 50;
const LONG_PRESS_MS: u64 = 1000; // 1 second for long press
const ADC_CHANNELS: usize = 8;
const DISPLAY_CHANNELS: [usize; 5] = [0, 1, 2, 6, 7];
const BUTTON_CHANGE_MODE: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ControlMode {
    Normal,
    Settings,
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
        
        // Check for long press on B2 to toggle mode
        if button_reader.is_button_long_press(BUTTON_CHANGE_MODE) && !mode_just_changed {
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
        } else if !button_reader.is_button_long_press(BUTTON_CHANGE_MODE) {
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
