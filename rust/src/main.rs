use rppal::gpio::{Gpio, InputPin, Level};
use std::thread;
use std::time::{Duration, Instant};

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15];
const DEBOUNCE_MS: u64 = 50; // Debounce time in milliseconds

#[derive(Debug, Clone, Copy, PartialEq)]
enum Edge {
    Rising,   // Transition from Low to High (button pressed)
    Falling,  // Transition from High to Low (button released)
}

struct ButtonState {
    current: Level,
    last_stable: Level,
    last_change: Instant,
}

impl ButtonState {
    fn new() -> Self {
        ButtonState {
            current: Level::Low,
            last_stable: Level::Low,
            last_change: Instant::now(),
        }
    }

    fn update(&mut self, new_level: Level) -> Option<Edge> {
        if new_level != self.current {
            self.current = new_level;
            self.last_change = Instant::now();
            return None; // Unstable state, waiting for debounce
        }

        // If state is stable for DEBOUNCE_MS and different from last stable state
        if self.last_change.elapsed() >= Duration::from_millis(DEBOUNCE_MS)
            && self.current != self.last_stable
        {
            let edge = if self.current == Level::High {
                Some(Edge::Rising)
            } else {
                Some(Edge::Falling)
            };
            self.last_stable = self.current;
            return edge;
        }

        None
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

    fn display_state(&self, states: &[Level], edges: &[Option<Edge>]) {
        print!("\r");
        for (i, (&level, &edge)) in states.iter().zip(edges.iter()).enumerate() {
            let status = match level {
                Level::High => "HIGH",
                Level::Low => "LOW ",
            };
            let edge_info = match edge {
                Some(Edge::Rising) => " ^ ",
                Some(Edge::Falling) => " v ",
                None => "   ",
            };
            print!("Btn{}: {}{} | ", i + 1, status, edge_info);
        }
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
    }

    fn log_events(&self, edges: &[Option<Edge>]) {
        for (i, &edge) in edges.iter().enumerate() {
            if let Some(e) = edge {
                let event_type = match e {
                    Edge::Rising => "PRESSED",
                    Edge::Falling => "RELEASED",
                };
                println!(
                    "\n[{}] Button {} {} (GPIO {})",
                    chrono::Local::now().format("%H:%M:%S%.3f"),
                    i + 1,
                    event_type,
                    BUTTON_PINS[i]
                );
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Button Reader - Pins: {:?} - Debounce: {}ms\n", BUTTON_PINS, DEBOUNCE_MS);

    let mut reader = ButtonReader::new(&BUTTON_PINS)?;

    loop {
        let edges = reader.read_and_detect_edges();
        let states = reader.get_current_states();
        
        reader.display_state(&states, &edges);
        reader.log_events(&edges);
        
        thread::sleep(Duration::from_millis(10));
    }
}
