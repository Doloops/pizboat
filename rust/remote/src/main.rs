mod display;
mod config;
mod adc;
mod buttons;
mod websocket;

use websocket::{websocket_thread, CommandMessage};
use config::{Settings};
use display::{DisplayData, display_thread};
use adc::AdcReader;
use buttons::{ButtonReader, Edge};

use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15];

const ADC_CHANNELS: usize = 8;
// const DISPLAY_CHANNELS: [usize; 5] = [0, 1, 2, 6, 7];



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
    
    thread::spawn(move || {
        display_thread(rx_display);
    });

    let data_mutex: Arc<Mutex<Option<CommandMessage>>> = Arc::new(Mutex::new(None));

    let data_mutex_clone = Arc::clone(&data_mutex);
    thread::spawn(move || {
        websocket_thread(data_mutex_clone);
    });

    let mut settings = Settings::new();
        

    loop {
        handle_buttons_for_settings(&mut settings, &mut button_reader);
        
        let adc_values = adc_reader.read_all_channels()?;

        // Transform ADC values (rudder on channel 0, motor on channel 1)
        let rudder_star = settings.channels[0].transform_adc(adc_values[6]);
        let rudder_port = settings.channels[1].transform_adc(adc_values[6]);
        let motor_value = settings.channels[2].transform_adc(adc_values[7]);

        let button_states = button_reader.get_current_states();
        
        let display_data = DisplayData {
            settings: settings.clone(),
            rudder_star,
            rudder_port,
            motor_value,
        };
        let _ = tx_display.try_send(display_data);
        
        let command_message = CommandMessage {
            msg_type: String::from("command"),
            timestamp: 823,
            rudder_star,
            rudder_port,
            motor: motor_value,
            sail: 1500,
            genoa: 1500
        };
        
        {
            let mut locked_data = data_mutex.lock().unwrap();
            *locked_data = Some(command_message);
        }
        
        thread::sleep(Duration::from_millis(40));
    }
}
