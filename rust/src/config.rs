use serde::{Serialize, Deserialize};

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
    pub min: u16,         // Minimum output value
    pub max: u16,         // Maximum output value
    pub center: u16,      // Center output value
}

impl ChannelConfig {
    fn new(_name: &'static str) -> Self {
        ChannelConfig {
            name: String::from(_name),
            deadzone: 50,
            min: 1000,
            max: 2000,
            center: 1500,
        }
    }
}

impl ChannelConfig {
    pub fn transform_adc(&self, adc_value: u16) -> u16 {
        let center_adc = 512;
        let adc = adc_value as i32;
        let center = center_adc as i32;
        
        // Apply deadzone
        if (adc - center).abs() < self.deadzone as i32 {
            return self.center;
        }
        
        // Map ADC range to output range
        if adc > center {
            // Above center: map [center+deadzone, 1023] to [center, max]
            let adc_range = 1023 - (center + self.deadzone as i32);
            let out_range = self.max as i32 - self.center as i32;
            let normalized = (adc - center - self.deadzone as i32).max(0);
            let output = self.center as i32 + (normalized * out_range / adc_range);
            output.clamp(self.center as i32, self.max as i32) as u16
        } else {
            // Below center: map [0, center-deadzone] to [min, center]
            let adc_range = center - self.deadzone as i32;
            let out_range = self.center as i32 - self.min as i32;
            let normalized = (center - self.deadzone as i32 - adc).max(0);
            let output = self.center as i32 - (normalized * out_range / adc_range);
            output.clamp(self.min as i32, self.center as i32) as u16
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
    pub channels: Vec<ChannelConfig>,
    current_channel: usize,
    pub current_value: SettingsValue
}

impl Settings {
    pub fn new() -> Self {
        let mut channels = Vec::new();
        channels.push(ChannelConfig::new("Rudder"));
        channels.push(ChannelConfig::new("Motor"));
        
        Settings{mode: ControlMode::Normal, channels: channels, current_channel: 0, current_value: SettingsValue::Deadzone}
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
            SettingsValue::Deadzone => SettingsValue::Max,
            SettingsValue::Min => SettingsValue::Deadzone,
            SettingsValue::Max => SettingsValue::Min
        }
    }
    
    fn next_value(&mut self) {
        self.current_value = match self.current_value {
            SettingsValue::Deadzone => SettingsValue::Min,
            SettingsValue::Min => SettingsValue::Max,
            SettingsValue::Max => SettingsValue::Deadzone
        }
    }
    
    pub fn get_value(&self) -> u16 {
        match self.current_value {
        SettingsValue::Deadzone => self.current_channel().deadzone,
        SettingsValue::Min => self.current_channel().min,
        SettingsValue::Max => self.current_channel().max,
        }
    }
    
    fn add_value(&mut self, diff: u16) {
        match self.current_value {
        SettingsValue::Deadzone => { self.mut_current_channel().deadzone += diff; }
        SettingsValue::Min => { self.mut_current_channel().min += diff; }
        SettingsValue::Max => { self.mut_current_channel().max += diff; }
        }
    }

    fn sub_value(&mut self, diff: u16) {
        match self.current_value {
        SettingsValue::Deadzone => { self.mut_current_channel().deadzone -= diff; }
        SettingsValue::Min => { self.mut_current_channel().min -= diff; }
        SettingsValue::Max => { self.mut_current_channel().max -= diff; }
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
    
}
    

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SettingsValue {
    Deadzone,
    Min,
    Max
}
