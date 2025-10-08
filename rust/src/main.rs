use rppal::gpio::{Gpio, InputPin, Level};
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use rppal::i2c::I2c;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc::{self, SyncSender, Receiver};
use std::sync::{Arc, Mutex};
use std::net::TcpListener;
use serde::{Serialize, Deserialize};
use tungstenite::{accept, Message};

const BUTTON_PINS: [u8; 6] = [12, 25, 24, 23, 18, 15];
const DEBOUNCE_MS: u64 = 50;
const ADC_CHANNELS: usize = 8;
const DISPLAY_CHANNELS: [usize; 5] = [0, 1, 2, 6, 7];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Edge {
    Rising,
    Falling,
}

#[derive(Clone, Serialize, Deserialize)]
struct DisplayData {
    adc_values: [u16; ADC_CHANNELS],
    button_states: [bool; 6],
    last_event: String,
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
            return None;
        }

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
}

struct DisplayBuffer {
    buffer: [u8; 1024],
}

impl DisplayBuffer {
    fn new() -> Self {
        DisplayBuffer {
            buffer: [0u8; 1024],
        }
    }

    fn clear(&mut self) {
        self.buffer.fill(0);
    }

    fn set_pixel(&mut self, x: u8, y: u8, on: bool) {
        if x >= 128 || y >= 64 {
            return;
        }
        let byte_index = (y / 8) as usize * 128 + x as usize;
        let bit_index = y % 8;
        
        if on {
            self.buffer[byte_index] |= 1 << bit_index;
        } else {
            self.buffer[byte_index] &= !(1 << bit_index);
        }
    }

    fn draw_char(&mut self, x: u8, y: u8, c: char) {
        let font = get_font_data(c);
        for dx in 0..5u8 {
            let column = font[dx as usize];
            for dy in 0..8u8 {
                if (column >> dy) & 1 == 1 {
                    self.set_pixel(x + dx, y + dy, true);
                }
            }
        }
    }

    fn draw_text(&mut self, x: u8, y: u8, text: &str) {
        for (i, c) in text.chars().enumerate() {
            self.draw_char(x + (i as u8 * 6), y, c);
        }
    }
}

struct SSD1306 {
    i2c: I2c,
}

impl SSD1306 {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut i2c = I2c::with_bus(1)?;
        i2c.set_slave_address(0x3C)?;
        
        let mut display = SSD1306 { i2c };
        display.init()?;
        
        println!("SSD1306 OLED initialized on I2C bus 1, address 0x3C");
        Ok(display)
    }

    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let init_commands = [
            0xAE, 0xD5, 0x80, 0xA8, 0x3F, 0xD3, 0x00, 0x40,
            0x8D, 0x14, 0x20, 0x00, 0xA1, 0xC8, 0xDA, 0x12,
            0x81, 0xCF, 0xD9, 0xF1, 0xDB, 0x40, 0xA4, 0xA6, 0xAF,
        ];

        for &cmd in &init_commands {
            self.send_command(cmd)?;
        }

        Ok(())
    }

    fn send_command(&mut self, cmd: u8) -> Result<(), Box<dyn std::error::Error>> {
        self.i2c.write(&[0x00, cmd])?;
        Ok(())
    }

    fn display(&mut self, buffer: &DisplayBuffer) -> Result<(), Box<dyn std::error::Error>> {
        self.send_command(0x21)?;
        self.send_command(0)?;
        self.send_command(127)?;
        self.send_command(0x22)?;
        self.send_command(0)?;
        self.send_command(7)?;

        for chunk in buffer.buffer.chunks(16) {
            let mut data = vec![0x40];
            data.extend_from_slice(chunk);
            self.i2c.write(&data)?;
        }

        Ok(())
    }
}

fn get_font_data(c: char) -> [u8; 5] {
    match c {
        '0' => [0x3E, 0x51, 0x49, 0x45, 0x3E],
        '1' => [0x00, 0x42, 0x7F, 0x40, 0x00],
        '2' => [0x62, 0x51, 0x49, 0x49, 0x46],
        '3' => [0x22, 0x41, 0x49, 0x49, 0x36],
        '4' => [0x18, 0x14, 0x12, 0x7F, 0x10],
        '5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        '6' => [0x3C, 0x4A, 0x49, 0x49, 0x30],
        '7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        '8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        '9' => [0x06, 0x49, 0x49, 0x29, 0x1E],
        'A' => [0x7C, 0x12, 0x11, 0x12, 0x7C],
        'B' => [0x7F, 0x49, 0x49, 0x49, 0x36],
        'C' => [0x3E, 0x41, 0x41, 0x41, 0x22],
        'D' => [0x7F, 0x41, 0x41, 0x41, 0x3E],
        'E' => [0x7F, 0x49, 0x49, 0x49, 0x41],
        'L' => [0x7F, 0x40, 0x40, 0x40, 0x40],
        'N' => [0x7F, 0x02, 0x04, 0x08, 0x7F],
        'P' => [0x7F, 0x09, 0x09, 0x09, 0x06],
        'R' => [0x7F, 0x09, 0x19, 0x29, 0x46],
        'S' => [0x26, 0x49, 0x49, 0x49, 0x32],
        'V' => [0x03, 0x04, 0x78, 0x04, 0x03],
        'T' => [0x01, 0x01, 0x7F, 0x01, 0x01],
        ':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00],
        '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        _ => [0x7F, 0x41, 0x41, 0x41, 0x7F],
    }
}

fn display_thread(rx: Receiver<DisplayData>) {
    let mut display = match SSD1306::new() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to initialize display: {}", e);
            return;
        }
    };

    let mut display_buffer = DisplayBuffer::new();
    let mut current_data: Option<DisplayData> = None;

    loop {
        match rx.try_recv() {
            Ok(data) => {
                current_data = Some(data);
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {
                thread::sleep(Duration::from_millis(50));
            }
        }

        if let Some(ref data) = current_data {
            display_buffer.clear();
            
            for (line, &channel) in DISPLAY_CHANNELS.iter().enumerate() {
                let y = (line * 10) as u8;
                let text = format!("ADC{}:{}", channel, data.adc_values[channel]);
                display_buffer.draw_text(0, y, &text);
            }

            display_buffer.draw_text(0, 54, "EVENT:");
            display_buffer.draw_text(42, 54, &data.last_event);

            if let Err(e) = display.display(&display_buffer) {
                eprintln!("Display error: {}", e);
            }
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
    println!("Starting Button and ADC Reader with Threaded Display and WebSocket");

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

    loop {
        let edges = button_reader.read_and_detect_edges();
        let button_states = button_reader.get_current_states();
        
        for (i, &edge) in edges.iter().enumerate() {
            if let Some(e) = edge {
                let event_type = match e {
                    Edge::Rising => "PR",
                    Edge::Falling => "RL",
                };
                last_event = format!("B{} {}", i, event_type);
                println!("[EVENT] Button {} {} (GPIO {})", i, event_type, BUTTON_PINS[i]);
            }
        }

        let adc_values = adc_reader.read_all_channels()?;

        let button_states_bool: [bool; 6] = [
            button_states[0] == Level::High,
            button_states[1] == Level::High,
            button_states[2] == Level::High,
            button_states[3] == Level::High,
            button_states[4] == Level::High,
            button_states[5] == Level::High,
        ];

        let display_data = DisplayData {
            adc_values,
            button_states: button_states_bool,
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
