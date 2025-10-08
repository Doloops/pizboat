use rppal::gpio::{Gpio, InputPin, Level};
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use std::thread;
use std::time::{Duration, Instant};

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15];
const DEBOUNCE_MS: u64 = 50; // Debounce time in milliseconds
const ADC_CHANNELS: usize = 8; // MCP3008 has 8 channels

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

struct AdcReader {
    spi: Spi,
}

impl AdcReader {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Configure SPI for MCP3008
        // Clock speed: 1 MHz (MCP3008 max is 3.6 MHz at 5V, 1.35 MHz at 2.7V)
        let spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, 488_000, Mode::Mode0)?;
        println!("MCP3008 ADC initialized on SPI0.0");
        Ok(AdcReader { spi })
    }

    fn read_channel(&mut self, channel: u8) -> Result<u16, Box<dyn std::error::Error>> {
        if channel >= 8 {
            return Err("Channel must be 0-7".into());
        }

        // MCP3008 communication protocol
        // Send 3 bytes: [start bit + single-ended, channel selection, don't care]
        // Receive 3 bytes with 10-bit result
        let tx_buffer = [
            0x01,                           // Start bit
            (0x08 | channel) << 4,          // Single-ended mode + channel
            0x00,                           // Don't care
        ];
        let mut rx_buffer = [0u8; 3];

        self.spi.transfer(&mut rx_buffer, &tx_buffer)?;
        
        let buffer = rx_buffer;

        // Extract 10-bit value from response
        // Result is in bits [9:0] of bytes [1:2]
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
            print!("{}: {}{} | ", i + 1, status, edge_info);
        }
        use std::io::{self, Write};
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

fn display_adc_values(values: &[u16; ADC_CHANNELS]) {
    print!(" ADC: ");
    for (i, &value) in values.iter().enumerate() {
        let voltage = (value as f32 / 1023.0) * 3.3; // Assuming 3.3V reference
        print!("CH{}: {:4} ({:.2}V) | ", i, value, voltage);
    }
    use std::io::{self, Write};
    io::stdout().flush().unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Button and ADC Reader - Buttons: {:?} - Debounce: {}ms\n", BUTTON_PINS, DEBOUNCE_MS);

    let mut button_reader = ButtonReader::new(&BUTTON_PINS)?;
    let mut adc_reader = AdcReader::new()?;

    loop {
        let edges = button_reader.read_and_detect_edges();
        let button_states = button_reader.get_current_states();
        
        // io::stdout().flush().unwrap();

        print!("\r");
        
        // button_reader.display_state(&button_states, &edges);
        button_reader.log_events(&edges);

        // Read ADC values
        match adc_reader.read_all_channels() {
            Ok(adc_values) => display_adc_values(&adc_values),
            Err(e) => print!(" ADC Error: {} ", e),
        }
        
        thread::sleep(Duration::from_millis(100)); // 100ms refresh rate
    }
}
