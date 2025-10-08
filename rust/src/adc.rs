use rppal::spi::{Bus, Mode, SlaveSelect, Spi};

pub struct AdcReader {
    spi: Spi,
}

impl AdcReader {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
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

    pub fn read_all_channels(&mut self) -> Result<[u16; crate::ADC_CHANNELS], Box<dyn std::error::Error>> {
        let mut values = [0u16; crate::ADC_CHANNELS];
        for channel in 0..crate::ADC_CHANNELS {
            values[channel] = self.read_channel(channel as u8)?;
        }
        Ok(values)
    }
}
