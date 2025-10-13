use serde::{Serialize, Deserialize};
use std::fs;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ControlMode {
    Normal,
    Settings,
    SettingsValue
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub name: String,
    pub deadzone: u16,    // Deadzone around center (512)
    pub center: u16,      // Center output value
    pub min: u16,         // Minimum output value
    pub max: u16,         // Maximum output value
    pub step: u16,    // Maximum change in values between two updates
    
    previous_value: u16
}

impl ChannelConfig {
    fn new(_name: &'static str) -> Self {
        ChannelConfig {
            name: String::from(_name),
            deadzone: 50,
            min: 1000,
            max: 2000,
            center: 1500,
            step: 100,
            previous_value: 1500
        }
    }
}

impl ChannelConfig {
    pub fn transform_adc(&mut self, adc_value: u16) -> u16 {
        let center_adc = 512;
        let adc = adc_value as i32;
        let center = center_adc as i32;
        
        // Apply deadzone
        if (adc - center).abs() < self.deadzone as i32 {
            let output = self.center.clamp(self.previous_value - self.step, self.previous_value + self.step);
            self.previous_value = output;
            return output;
        }
        
        let mut output: u16;
        
        // Map ADC range to output range
        if adc > center {
            // Above center: map [center+deadzone, 1023] to [center, max]
            let adc_range = 1023 - (center + self.deadzone as i32);
            let out_range = self.max as i32 - self.center as i32;
            let normalized = (adc - center - self.deadzone as i32).max(0);
            output = (self.center as i32 + (normalized * out_range / adc_range)) as u16;
            // output = output.clamp(self.center as i32, self.max as i32) as u16
        } else {
            // Below center: map [0, center-deadzone] to [min, center]
            let adc_range = center - self.deadzone as i32;
            let out_range = self.center as i32 - self.min as i32;
            let normalized = (center - self.deadzone as i32 - adc).max(0);
            output = (self.center as i32 - (normalized * out_range / adc_range)) as u16;
            // output = output.clamp(self.min as i32, self.center as i32) as u16
        }
        
        output = output.clamp(self.min, self.max);
        output = output.clamp(self.previous_value - self.step, self.previous_value + self.step);
        
        self.previous_value = output;
        
        return output
    }
    
    pub fn apply_button(&self, up: bool, down: bool, adc_value: u16) -> u16 {
        let out_range = self.max as u32 - self.min as u32;
        let diff = ((adc_value as u32 * out_range) / 1024) as u16;
        
        // eprintln!("adc_value {} diff {}", adc_value, diff);
        
        if up {
            self.center + diff
        }
        else if down {
            self.center - diff
        }
        else {
            self.center
        }
    }
}



const BUTTON_CANCEL_MODE: usize = 0;
const BUTTON_UP: usize = 1;
const BUTTON_CHANGE_MODE: usize = 2;
const BUTTON_LEFT: usize = 3;
const BUTTON_DOWN: usize = 4;
const BUTTON_RIGHT: usize = 5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub mode: ControlMode,
    settings_path: String,
    pub channels: Vec<ChannelConfig>,
    current_channel: usize,
    pub current_value: SettingsValue
}

impl Settings {
    pub fn new(settings_path: &'static str) -> Self {
        let mut channels = Vec::new();
        channels.push(ChannelConfig::new("RudderStar"));
        channels.push(ChannelConfig::new("RudderPort"));
        channels.push(ChannelConfig::new("Motor"));
        channels.push(ChannelConfig::new("Boom"));
        channels.push(ChannelConfig::new("Genoa"));
        
        Settings{mode: ControlMode::Normal, settings_path: settings_path.to_string(), channels: channels, current_channel: 0, current_value: SettingsValue::Deadzone}
    }
    
    fn previous_channel(&mut self) {
        self.current_channel = if self.current_channel == 0 { self.channels.len()-1 } else { self.current_channel - 1};
    }
    
    fn next_channel(&mut self) {
        self.current_channel = if self.current_channel == self.channels.len()-1 { 0 } else { self.current_channel + 1};
    }

    fn current_channel(&self) -> &ChannelConfig {
        &(self.channels[self.current_channel])
    }

    fn mut_current_channel(&mut self) -> &mut ChannelConfig {
        &mut(self.channels[self.current_channel])
    }
    
    pub fn current_channel_name(&self) -> String {
        self.channels[self.current_channel].name.clone()
    }
    
    fn previous_value(&mut self) {
        self.current_value = match self.current_value {
            SettingsValue::Deadzone => SettingsValue::Step,
            SettingsValue::Center => SettingsValue::Deadzone,
            SettingsValue::Min => SettingsValue::Center,
            SettingsValue::Max => SettingsValue::Min,
            SettingsValue::Step => SettingsValue::Max
        }
    }
    
    fn next_value(&mut self) {
        self.current_value = match self.current_value {
            SettingsValue::Deadzone => SettingsValue::Center,
            SettingsValue::Center => SettingsValue::Min,
            SettingsValue::Min => SettingsValue::Max,
            SettingsValue::Max => SettingsValue::Step,
            SettingsValue::Step => SettingsValue::Deadzone
        }
    }
    
    pub fn get_value(&self) -> u16 {
        match self.current_value {
        SettingsValue::Deadzone => self.current_channel().deadzone,
        SettingsValue::Center => self.current_channel().center,
        SettingsValue::Min => self.current_channel().min,
        SettingsValue::Max => self.current_channel().max,
        SettingsValue::Step => self.current_channel().step,
        }
    }
    
    fn add_value(&mut self, diff: u16) {
        match self.current_value {
        SettingsValue::Deadzone => { self.mut_current_channel().deadzone += diff; }
        SettingsValue::Center => { self.mut_current_channel().center += diff; }
        SettingsValue::Min => { self.mut_current_channel().min += diff; }
        SettingsValue::Max => { self.mut_current_channel().max += diff; }
        SettingsValue::Step => { self.mut_current_channel().step += 1; }
        }
    }

    fn sub_value(&mut self, diff: u16) {
        match self.current_value {
        SettingsValue::Deadzone => { self.mut_current_channel().deadzone -= diff; }
        SettingsValue::Center => { self.mut_current_channel().center -= diff; }
        SettingsValue::Min => { self.mut_current_channel().min -= diff; }
        SettingsValue::Max => { self.mut_current_channel().max -= diff; }
        SettingsValue::Step => { self.mut_current_channel().step -= 1; }
        }
    }
    
    pub fn handle_button(&mut self, button: usize) {
        match button {
            BUTTON_CHANGE_MODE => {
                let previous_mode = self.mode;
                self.mode = match self.mode {
                    ControlMode::Normal => {
                        ControlMode::Settings
                    }
                    ControlMode::Settings => {
                        ControlMode::SettingsValue
                    }
                    ControlMode::SettingsValue => {
                        let _ = self.save();
                        ControlMode::Settings
                    }
                };
                println!("[Changed mode {:?} => {:?}", previous_mode, self.mode);
            }
            BUTTON_CANCEL_MODE => {
                let previous_mode = self.mode;
                self.mode = match self.mode {
                    ControlMode::Normal => {
                        ControlMode::Normal
                    }
                    ControlMode::Settings => {
                        ControlMode::Normal
                    }
                    ControlMode::SettingsValue => {
                        ControlMode::Settings
                    }
                };
                println!("[Changed mode {:?} => {:?}", previous_mode, self.mode);
            }
            BUTTON_LEFT => {
                match self.mode {
                    ControlMode::Normal => {
                    }
                    ControlMode::Settings => {
                        self.previous_channel();
                    }
                    ControlMode::SettingsValue => {
                        self.previous_value();
                    }
                }
            }
            BUTTON_RIGHT => {
                match self.mode {
                    ControlMode::Normal => {
                    }
                    ControlMode::Settings => {
                        self.next_channel();
                    }
                    ControlMode::SettingsValue => {
                        self.next_value();
                    }
                }
            }
            BUTTON_UP => {
                match self.mode {
                    ControlMode::SettingsValue => {
                        self.add_value(10);
                    }
                    _ => {}
                }
            }
            BUTTON_DOWN => {
                match self.mode {
                    ControlMode::SettingsValue => {
                        self.sub_value(10);
                    }
                    _ => {}
                }
            }
            _ => {}
        };        
        
    }
    
    pub fn save(&self) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        let mut file = fs::File::create(self.settings_path.clone())?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
    
    pub fn load(&mut self) -> io::Result<()> {
        let content = fs::read_to_string(self.settings_path.clone())?;
        let loaded: Settings = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        *self = loaded;
        Ok(())
    }
}
    

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SettingsValue {
    Deadzone,
    Center,
    Min,
    Max,
    Step
}
