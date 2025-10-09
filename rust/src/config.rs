use serde::{Serialize, Deserialize};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub channels: Vec<ChannelConfig>,
    currentChannel: usize,
    pub currentValue: SettingsValue
}

impl Settings {
    pub fn new() -> Self {
        let mut channels = Vec::new();
        channels.push(ChannelConfig::new("Rudder"));
        channels.push(ChannelConfig::new("Motor"));
        
        Settings{channels: channels, currentChannel: 0, currentValue: SettingsValue::Deadzone}
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
    
}
    

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SettingsValue {
    Deadzone,
    Min,
    Max
}



#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsParameter {
    RudderDeadzone,
    RudderMin,
    RudderMax,
    RudderCenter,
    MotorDeadzone,
    MotorMin,
    MotorMax,
    MotorCenter,
}

impl SettingsParameter {
    pub fn next(&self) -> Self {
        match self {
            Self::RudderDeadzone => Self::RudderMin,
            Self::RudderMin => Self::RudderMax,
            Self::RudderMax => Self::RudderCenter,
            Self::RudderCenter => Self::MotorDeadzone,
            Self::MotorDeadzone => Self::MotorMin,
            Self::MotorMin => Self::MotorMax,
            Self::MotorMax => Self::MotorCenter,
            Self::MotorCenter => Self::RudderDeadzone,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::RudderDeadzone => Self::MotorCenter,
            Self::RudderMin => Self::RudderDeadzone,
            Self::RudderMax => Self::RudderMin,
            Self::RudderCenter => Self::RudderMax,
            Self::MotorDeadzone => Self::RudderCenter,
            Self::MotorMin => Self::MotorDeadzone,
            Self::MotorMax => Self::MotorMin,
            Self::MotorCenter => Self::MotorMax,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::RudderDeadzone => "RUD DZ",
            Self::RudderMin => "RUD MIN",
            Self::RudderMax => "RUD MAX",
            Self::RudderCenter => "RUD CTR",
            Self::MotorDeadzone => "MOT DZ",
            Self::MotorMin => "MOT MIN",
            Self::MotorMax => "MOT MAX",
            Self::MotorCenter => "MOT CTR",
        }
    }

    pub fn get_value(&self, rudder_cfg: &ChannelConfig, motor_cfg: &ChannelConfig) -> u16 {
        match self {
            Self::RudderDeadzone => rudder_cfg.deadzone,
            Self::RudderMin => rudder_cfg.min,
            Self::RudderMax => rudder_cfg.max,
            Self::RudderCenter => rudder_cfg.center,
            Self::MotorDeadzone => motor_cfg.deadzone,
            Self::MotorMin => motor_cfg.min,
            Self::MotorMax => motor_cfg.max,
            Self::MotorCenter => motor_cfg.center,
        }
    }

    pub fn set_value(&self, rudder_cfg: &mut ChannelConfig, motor_cfg: &mut ChannelConfig, value: u16) {
        match self {
            Self::RudderDeadzone => rudder_cfg.deadzone = value,
            Self::RudderMin => rudder_cfg.min = value,
            Self::RudderMax => rudder_cfg.max = value,
            Self::RudderCenter => rudder_cfg.center = value,
            Self::MotorDeadzone => motor_cfg.deadzone = value,
            Self::MotorMin => motor_cfg.min = value,
            Self::MotorMax => motor_cfg.max = value,
            Self::MotorCenter => motor_cfg.center = value,
        }
    }
}

