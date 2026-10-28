mod adc;
mod buttons;
mod pigpiod;

use adc::AdcReader;
use buttons::{ButtonReader, Edge};

use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const BUTTON_PINS: [u8; 6] = [0, 25, 24, 23, 16, 15];

const RUDDER_ADC: usize = 6;
const MOTOR_ADC: usize = 7;

const MISC_PIN_OPEN: u8 = 13;
const MISC_PIN_CLOSE: u8 = 12;
const MISC_PINS: [u8; 2] = [MISC_PIN_OPEN, MISC_PIN_CLOSE];


const ADC_CHANNELS: usize = 8;

const BUTTON_MIDFIX_OPEN:    usize = 0;
const BUTTON_MIDFIX_CLOSE:   usize = 3;

const PERIOD_MS: u64 = 10;

const SWEEP_HZ: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidFixState {
    Unknown,
    Closing,
    Closed,
    Opening,
    Open,
}

const HALL_HIGH: u16 = 760;
const HALL_LOW: u16 = 250;

const PWM_LOW: u32 = 0;
const PWM_HIGH: u32 = 3000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    
    let mut midFixState = MidFixState::Unknown;

    println!("Starting RC Boat MidFix Test");

    let mut button_reader = ButtonReader::new(&BUTTON_PINS)?;
    let mut adc_reader = AdcReader::new()?;
    
    let mut pig = pigpiod::Pigpiod::connect("localhost:8888")?;
    println!("Connected to pigpiod. Version: {}", pig.version()?);
    
    let mut misc_pwms = MISC_PINS.map(|pin| {
        pig.set_mode(pin as u32, pigpiod::OUTPUT).unwrap();

        pig.set_pwm_frequency(pin as u32, 500).unwrap(); // Set to modulate at 500hz.
        let real_range = pig.set_pwm_range(pin as u32, 3000).unwrap(); // Set range to 1000. 1 range = 2 us;
        println!("PWM pin {} real range {}", pin, real_range);
    });
    

    pig.pwm(MISC_PIN_OPEN as u32, PWM_LOW)?;
    pig.pwm(MISC_PIN_CLOSE as u32, PWM_LOW)?;

    
    loop {
        let adc_values = adc_reader.read_all_channels()?;

        let hallValue = adc_values[5];

        print!("Hall {} ", hallValue);
        
        if midFixState == MidFixState::Unknown
        {
            if hallValue < 450
            {
                midFixState = MidFixState::Closed;
            }
            else if hallValue > 550
            {
                midFixState = MidFixState::Open;
            }
            else
            {
                midFixState = MidFixState::Closing;
                // return Err(format!("Cannot infer MidFix state from hall sensor value {}", hallValue).into());
            }
        }

        print!("State {:?} ", midFixState);

        button_reader.read_and_detect_edges();
        let button_states = button_reader.get_current_states();
        
        // println!("previous_mode {:?} mode {:?} button_states[0] = {}", previous_mode, settings.mode, button_states[0]);
        
        // let boom = settings.channels[3].apply_button(button_states[BUTTON_BOOM_UP], button_states[BUTTON_BOOM_DOWN], adc_values[1]);
        // let genoa = settings.channels[4].apply_button(button_states[BUTTON_GENOA_UP], button_states[BUTTON_GENOA_DOWN], adc_values[0]);

        let button_open = button_states[BUTTON_MIDFIX_OPEN];
        let button_close = button_states[BUTTON_MIDFIX_CLOSE];
        
        print!("| Buttons OPEN {:?} CLOSE {:?}", button_open, button_close);
        
        match midFixState {
            MidFixState::Unknown => {
                return Err(format!("Cannot infer MidFix state from hall sensor value {}", hallValue).into());
            }
            MidFixState::Closing => {
                if button_open {
                    midFixState = MidFixState::Opening;
                    pig.pwm(MISC_PIN_CLOSE as u32, PWM_LOW)?;
                }
                else if hallValue < HALL_LOW {
                    midFixState = MidFixState::Closed;
                    pig.pwm(MISC_PIN_CLOSE as u32, PWM_LOW)?;
                }
                else {
                    pig.pwm(MISC_PIN_CLOSE as u32, PWM_HIGH)?;
                }
            }
            MidFixState::Closed => {
                if button_open {
                    midFixState = MidFixState::Opening;
                }
                if button_close {
                    pig.pwm(MISC_PIN_CLOSE as u32, PWM_HIGH)?;
                }
                else {
                    pig.pwm(MISC_PIN_CLOSE as u32, PWM_LOW)?;
                }
            }
            MidFixState::Opening => {
                if button_close {
                    midFixState = MidFixState::Closing;
                    pig.pwm(MISC_PIN_OPEN as u32, PWM_LOW)?;
                }
                else if hallValue > HALL_HIGH {
                    midFixState = MidFixState::Open;
                    pig.pwm(MISC_PIN_OPEN as u32, PWM_LOW)?;
                }
                else {
                    pig.pwm(MISC_PIN_OPEN as u32, PWM_HIGH)?;
                }
            }
            MidFixState::Open => {
                if button_close {
                    midFixState = MidFixState::Closing;
                }
                if button_open {
                    pig.pwm(MISC_PIN_OPEN as u32, PWM_HIGH)?;
                }
                else {
                    pig.pwm(MISC_PIN_OPEN as u32, PWM_LOW)?;
                }
            }            
        }
        
        println!(".");
        
        thread::sleep(Duration::from_millis(40));
    }
}
