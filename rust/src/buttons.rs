use rppal::gpio::{Gpio, InputPin, Level};
use std::time::{Duration, Instant};


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
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

        if self.last_change.elapsed() >= Duration::from_millis(crate::DEBOUNCE_MS)
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
                return start.elapsed() >= Duration::from_millis(crate::LONG_PRESS_MS);
            }
        }
        false
    }
}

pub struct ButtonReader {
    pins: Vec<InputPin>,
    states: Vec<ButtonState>,
}

impl ButtonReader {
    pub fn new(pin_numbers: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
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

    pub fn read_and_detect_edges(&mut self) -> Vec<Option<Edge>> {
        self.pins
            .iter()
            .enumerate()
            .map(|(i, pin)| {
                let level = pin.read();
                self.states[i].update(level)
            })
            .collect()
    }

    pub fn get_current_states(&self) -> Vec<Level> {
        self.states.iter().map(|s| s.last_stable).collect()
    }

    pub fn is_button_long_press(&self, button_index: usize) -> bool {
        if button_index < self.states.len() {
            self.states[button_index].is_long_press()
        } else {
            false
        }
    }
}
