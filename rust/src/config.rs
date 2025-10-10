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
    currentChannel: usize,
    pub currentValue: SettingsValue
}

impl Settings {
    pub fn new() -> Self {
        let mut channels = Vec::new();
        channels.push(ChannelConfig::new("Rudder"));
        channels.push(ChannelConfig::new("Motor"));
        
        Settings{mode: ControlMode::Normal, channels: channels, currentChannel: 0, currentValue: SettingsValue::Deadzone}
    }
    
    pub fn firstChannel(&mut self) {
        self.currentChannel = 0;
        self.currentValue = SettingsValue::Deadzone;
    }
    
    pub fn previousChannel(&mut self) {
        self.currentChannel = if self.currentChannel == 0 { self.channels.len()-1 } else { self.currentChannel - 1};
    }
    
    pub fn nextChannel(&mut self) {
        self.currentChannel = if self.currentChannel == self.channels.len()-1 { 0 } else { self.currentChannel + 1};
    }

    fn currentChannel(&self) -> &ChannelConfig {
        &(self.channels[self.currentChannel])
    }

    fn mutCurrentChannel(&mut self) -> &mut ChannelConfig {
        &mut(self.channels[self.currentChannel])
    }
    
    pub fn currentChannelName(&self) -> String {
        self.channels[self.currentChannel].name.clone()
    }
    
    pub fn previousValue(&mut self) {
        self.currentValue = match self.currentValue {
            SettingsValue::Deadzone => SettingsValue::Max,
            SettingsValue::Min => SettingsValue::Deadzone,
            SettingsValue::Max => SettingsValue::Min
        }
    }
    
    pub fn nextValue(&mut self) {
        self.currentValue = match self.currentValue {
            SettingsValue::Deadzone => SettingsValue::Min,
            SettingsValue::Min => SettingsValue::Max,
            SettingsValue::Max => SettingsValue::Deadzone
        }
    }
    
    pub fn getValue(&self) -> u16 {
        match self.currentValue {
        SettingsValue::Deadzone => self.currentChannel().deadzone,
        SettingsValue::Min => self.currentChannel().min,
        SettingsValue::Max => self.currentChannel().max,
        }
    }
    
    pub fn addValue(&mut self, diff: u16) {
        match self.currentValue {
        SettingsValue::Deadzone => { self.mutCurrentChannel().deadzone += diff; }
        SettingsValue::Min => { self.mutCurrentChannel().min += diff; }
        SettingsValue::Max => { self.mutCurrentChannel().max += diff; }
        }
    }

    pub fn subValue(&mut self, diff: u16) {
        match self.currentValue {
        SettingsValue::Deadzone => { self.mutCurrentChannel().deadzone -= diff; }
        SettingsValue::Min => { self.mutCurrentChannel().min -= diff; }
        SettingsValue::Max => { self.mutCurrentChannel().max -= diff; }
        }
    }
    
    pub fn handle_button(&mut self, button: usize) {
        match button {
            BUTTON_CHANGE_MODE => {
                let previous_mode = self.mode;
                self.mode = match self.mode {
                    ControlMode::Normal => {
                        self.firstChannel();
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
                        self.previousChannel();
                    }
                    ControlMode::SettingsValue => {
                        self.previousValue();
                    }
                }
            }
            BUTTON_RIGHT => {
                match self.mode {
                    ControlMode::Normal => {
                    }
                    ControlMode::Settings => {
                        self.nextChannel();
                    }
                    ControlMode::SettingsValue => {
                        self.nextValue();
                    }
                }
            }
            BUTTON_UP => {
                match self.mode {
                    ControlMode::SettingsValue => {
                        self.addValue(10);
                    }
                    _ => {}
                }
            }
            BUTTON_DOWN => {
                match self.mode {
                    ControlMode::SettingsValue => {
                        self.subValue(10);
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
