mod display;
mod config;
mod adc;
mod buttons;
mod websocket;
mod state;

use state::InternalState;
use websocket::websocket_thread;
use config::{Settings};
use display::{DisplayData, display_thread};
use adc::AdcReader;
use buttons::{ButtonReader, Edge};

use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15];
const DEBOUNCE_MS: u64 = 50;
const LONG_PRESS_MS: u64 = 1000; // 1 second for long press
const ADC_CHANNELS: usize = 8;
const DISPLAY_CHANNELS: [usize; 5] = [0, 1, 2, 6, 7];



fn handle_buttons_for_settings(settings: &mut Settings, button_reader: &mut ButtonReader) {
    let edges = button_reader.read_and_detect_edges();
        
    // Handle button events based on mode
    for (i, &edge) in edges.iter().enumerate() {
        if let Some(Edge::Rising) = edge {
            println!("[EVENT] Button {} pressed in mode {:?}", i, settings.mode);
            settings.handle_button(i);         
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting RC Boat Controller with WebSocket");

    let mut button_reader = ButtonReader::new(&BUTTON_PINS)?;
    let mut adc_reader = AdcReader::new()?;

    let (tx_display, rx_display): (SyncSender<DisplayData>, Receiver<DisplayData>) = mpsc::sync_channel(1);
    let data_mutex: Arc<Mutex<Option<InternalState>>> = Arc::new(Mutex::new(None));
    
    thread::spawn(move || {
        display_thread(rx_display);
    });

    let data_mutex_clone = Arc::clone(&data_mutex);
    thread::spawn(move || {
        websocket_thread(data_mutex_clone);
    });

    let mut settings = Settings::new();

    let mut last_event = String::from("--");
    // let mut rudder_config = ChannelConfig::default();
    // let mut motor_config = ChannelConfig::default();
    
    // Settings mode temporary variables
    // let mut temp_rudder_config = rudder_config;
    // let mut temp_motor_config = motor_config;
    // let mut selected_parameter = SettingsParameter::RudderDeadzone;
    
    let mut mode_just_changed = false;

    loop {
        
        let button_states = button_reader.get_current_states();

        /*
        
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

        let display_data = InternalState {
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
        */
        
        let display_data = DisplayData {
            settings: settings.clone()
        };
        
        let _ = tx_display.try_send(display_data);
        
        thread::sleep(Duration::from_millis(40));
    }
}
