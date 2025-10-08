use crate::pzconfig::ChannelConfig;
use serde::{Serialize, Deserialize};
use rppal::i2c::I2c;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration};

#[derive(Clone, Serialize, Deserialize)]
pub struct DisplayData {
    pub adc_values: [u16; crate::ADC_CHANNELS],
    pub button_states: [bool; 6],
    pub buttons_sent: [bool; 4], // B0, B1, B3, B4 in normal mode
    pub rudder_value: u16,       // Transformed rudder value (ADC channel 0)
    pub motor_value: u16,        // Transformed motor value (ADC channel 1)
    pub mode: String,
    pub rudder_config: ChannelConfig,
    pub motor_config: ChannelConfig,
    pub current_parameter: Option<String>,
    pub current_value: Option<u16>,
    pub last_event: String,
}


pub struct DisplayBuffer {
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

pub struct SSD1306 {
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
        'M' => [0x7F, 0x02, 0x0C, 0x02, 0x7F],
        'N' => [0x7F, 0x02, 0x04, 0x08, 0x7F],
        'O' => [0x3E, 0x41, 0x41, 0x41, 0x3E],
        'P' => [0x7F, 0x09, 0x09, 0x09, 0x06],
        'R' => [0x7F, 0x09, 0x19, 0x29, 0x46],
        'S' => [0x26, 0x49, 0x49, 0x49, 0x32],
        'T' => [0x01, 0x01, 0x7F, 0x01, 0x01],
        'U' => [0x3F, 0x40, 0x40, 0x40, 0x3F],
        'V' => [0x03, 0x04, 0x78, 0x04, 0x03],
        'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
        'K' => [0x7F, 0x08, 0x14, 0x22, 0x41],
        'G' => [0x3E, 0x41, 0x49, 0x49, 0x3A],
        'I' => [0x00, 0x41, 0x7F, 0x41, 0x00],
        ':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00],
        '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        _ => [0x7F, 0x41, 0x41, 0x41, 0x7F],
    }
}

pub fn display_thread(rx: Receiver<DisplayData>) {
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
            
            // Display mode on top
            display_buffer.draw_text(0, 0, &data.mode);
            
            if data.mode == "SETTINGS" {
                // Settings mode display
                if let (Some(param), Some(val)) = (&data.current_parameter, data.current_value) {
                    display_buffer.draw_text(0, 12, param);
                    let val_text = format!("VALUE:{}", val);
                    display_buffer.draw_text(0, 24, &val_text);
                    
                    // Show controls
                    display_buffer.draw_text(0, 40, "B1:UP B4:DN");
                    display_buffer.draw_text(0, 50, "B0:X B2:OK");
                }
            } else {
                // Normal mode display
                let rudder_text = format!("RUD:{}", data.rudder_value);
                display_buffer.draw_text(0, 10, &rudder_text);
                
                let motor_text = format!("MOT:{}", data.motor_value);
                display_buffer.draw_text(0, 20, &motor_text);
                
                // Display selected ADC channels
                for (line, &channel) in crate::DISPLAY_CHANNELS.iter().take(3).enumerate() {
                    let y = (30 + line * 10) as u8;
                    let text = format!("A{}:{}", channel, data.adc_values[channel]);
                    display_buffer.draw_text(0, y, &text);
                }
            }

            if let Err(e) = display.display(&display_buffer) {
                eprintln!("Display error: {}", e);
            }
        }
    }
}
