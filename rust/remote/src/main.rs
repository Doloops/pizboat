mod display;
mod config;
mod adc;
mod buttons;
mod websocket;
mod octled;

use websocket::{websocket_thread, CommandMessage, QueryMessage};
use config::{Settings, ControlMode};
use display::{DisplayData, display_thread};
use adc::AdcReader;
use buttons::{ButtonReader, Edge};
use octled::OctLed;

use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rppal::gpio::Gpio;

const BUTTON_PINS: [u8; 6] = [0, 25, 24, 23, 18, 15];
const LED_PINS: [u8; 8] = [16, 20, 21, 26, 19, 13, 6, 5];

const MISC_PIN: u8 = 12;
const MISC_ADC: usize = 2;

const ADC_CHANNELS: usize = 8;
// const DISPLAY_CHANNELS: [usize; 5] = [0, 1, 2, 6, 7];

fn handle_buttons_for_settings(settings: &mut Settings, button_reader: &mut ButtonReader) {
    let edges = button_reader.read_and_detect_edges();
        
    // Handle button events based on mode
    for (i, &edge) in edges.iter().enumerate() {
        if let Some(Edge::Falling) = edge {
            println!("[EVENT] Button {} pressed in mode {:?}", i, settings.mode);
            settings.handle_button(i);
        }
    }
}

const BUTTON_BOOM_UP:    usize = 0;
const BUTTON_BOOM_DOWN:  usize = 3;
const BUTTON_GENOA_UP:   usize = 1;
const BUTTON_GENOA_DOWN: usize = 4;

const PERIOD_MS: u64 = 20;
const PULSE_MIN_US: u64 = 1200;
const PULSE_NEUTRAL_US: u64 = 1500;
const PULSE_MAX_US: u64 = 1800;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting RC Boat Controller with WebSocket");

    let mut button_reader = ButtonReader::new(&BUTTON_PINS)?;
    let mut adc_reader = AdcReader::new()?;
    
    let mut led = OctLed::new(&LED_PINS)?;
    
    led.k2000();
    
    let (tx_display, rx_display): (SyncSender<DisplayData>, Receiver<DisplayData>) = mpsc::sync_channel(1);
    
    thread::spawn(move || {
        display_thread(rx_display);
    });

    let data_mutex: Arc<Mutex<Option<CommandMessage>>> = Arc::new(Mutex::new(None));
    let query_mutex: Arc<Mutex<Option<QueryMessage>>> = Arc::new(Mutex::new(None));

    let data_mutex_clone = Arc::clone(&data_mutex);
    let query_mutex_clone = Arc::clone(&query_mutex);
    thread::spawn(move || {
        websocket_thread(data_mutex_clone, query_mutex_clone);
    });

    let mut settings = Settings::new("settings.json");
    
    let zero_buttons = vec![false; 6];
    
    match settings.load() {
        Ok(_) => println!("Loaded successfully"),
        Err(e) => {
            println!("Error loading: {}", e);
        }
    }

    
    let mut misc_pwm = Gpio::new()?.get(MISC_PIN)?.into_output();
    

    settings.save()?;

    loop {
        let previous_mode = settings.mode;
        
        handle_buttons_for_settings(&mut settings, &mut button_reader);
        
        let adc_values = adc_reader.read_all_channels()?;

        // Transform ADC values (rudder on channel 0, motor on channel 1)
        let rudder_star = settings.channels[0].transform_adc(adc_values[6]);
        let rudder_port = settings.channels[1].transform_adc(adc_values[6]);
        let motor_value = settings.channels[2].transform_adc(adc_values[7]);
        
        // println!("adc 0 {} 1 {} 2 {} 6 {} 7 {}", adc_values[0], adc_values[1], adc_values[2], adc_values[6], adc_values[7]);

        let button_states = if previous_mode == ControlMode::Normal { button_reader.get_current_states() } else { zero_buttons.clone() };
        
        // println!("previous_mode {:?} mode {:?} button_states[0] = {}", previous_mode, settings.mode, button_states[0]);
        
        let boom = settings.channels[3].apply_button(button_states[BUTTON_BOOM_UP], button_states[BUTTON_BOOM_DOWN], adc_values[1]);
        let genoa = settings.channels[4].apply_button(button_states[BUTTON_GENOA_UP], button_states[BUTTON_GENOA_DOWN], adc_values[0]);
        
        let misc = settings.channels[5].transform_adc(adc_values[MISC_ADC]);
        
        let misc_width_us = misc.clamp(1000, 2000);

        println!("Servo at PIN {} sending {} (from {})", MISC_PIN, misc_width_us, adc_values[MISC_ADC]);

        misc_pwm.set_pwm(
            Duration::from_millis(PERIOD_MS),
            Duration::from_micros(misc_width_us.into()),
        )?;
       
        let mut wireless_quality: i16 = -1;
        let mut latency: u64 = 0;
        let mut weight: f32 = (-1) as f32;
        
        {
            match query_mutex.lock().unwrap().as_ref() {
                Some(query) => { 
                    wireless_quality = query.wireless_quality.unwrap_or(0-1);
                    latency = query.latency.unwrap_or(0);
                    /*
                    if wireless_quality > 0 && wireless_quality <= 70
                    {
                        let qual8 = (wireless_quality * 8) / 70;
                        led.display_value(qual8 as u8);
                    }
                    */
                    weight = query.weight.unwrap_or(0 as f32);
                    let w8 = ((weight * 8.) / 500.) as u8;
                    led.display_value(w8);
                }
                None => { }
            }
        }
        
        let display_data = DisplayData {
            settings: settings.clone(),
            rudder_star,
            rudder_port,
            motor_value,
            boom,
            genoa,
        
            wireless_quality,
            latency,
            weight
        };
        let _ = tx_display.try_send(display_data);
        
        
        let command_message = CommandMessage {
            msg_type: String::from("command"),
            timestamp: 0,
            rudder_star,
            rudder_port,
            motor: motor_value,
            boom,
            genoa
        };
        
        {
            let mut locked_data = data_mutex.lock().unwrap();
            *locked_data = Some(command_message);
        }
        
        thread::sleep(Duration::from_millis(40));
    }
}
