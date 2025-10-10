use serde::{Serialize, Deserialize};

// use crate::config::ChannelConfig;

#[derive(Clone, Serialize, Deserialize)]
pub struct InternalState {
    pub adc_values: [u16; crate::ADC_CHANNELS],
    pub button_states: [bool; 6],
    pub rudder_value: u16,       // Transformed rudder value (ADC channel 0)
    pub motor_value: u16,        // Transformed motor value (ADC channel 1)
    pub mode: String,
}
