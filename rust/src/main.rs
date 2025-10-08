use rppal::gpio::{Gpio, InputPin, Level};
use std::thread;
use std::time::Duration;

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15]; 

struct ButtonReader {
    pins: Vec<InputPin>,
}

impl ButtonReader {
    fn new(pin_numbers: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let gpio = Gpio::new()?;
        let mut pins = Vec::new();

        for &pin_num in pin_numbers {
            let pin = gpio.get(pin_num)?. into_input_pulldown();
            pins.push(pin);
        }

        Ok(ButtonReader { pins })
    }

    fn read_buttons(&self) -> Vec<Level> {
        self.pins
            .iter()
            .map(|pin| pin.read())
            .collect()
    }

    fn display_state(&self, states: &[Level]) {
        print!("\r");
        for (i, &pressed) in states.iter().enumerate() {
            print!("Btn{}: {} | ", i + 1, pressed);
        }
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reader = ButtonReader::new(&BUTTON_PINS)?;

    loop {
        let states = reader.read_buttons();
        reader.display_state(&states);
        thread::sleep(Duration::from_millis(50));
    }
}
