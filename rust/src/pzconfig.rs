use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub deadzone: u16,    // Deadzone around center (512)
    pub min: u16,         // Minimum output value
    pub max: u16,         // Maximum output value
    pub center: u16,      // Center output value
}

impl Default for ChannelConfig {
    fn default() -> Self {
        ChannelConfig {
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
