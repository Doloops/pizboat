// Minimal client for the pigpiod socket interface, see https://abyz.me.uk/rpi/pigpio/sif.html
// Each request is 4 x u32 (cmd, p1, p2, p3), the daemon echoes them back with the result in p3.
// No C library involved, so nothing to link when cross-compiling, and no root needed.

use std::io::{self, Read, Write};
use std::net::TcpStream;

const CMD_MODES: u32 = 0;
const CMD_WRITE: u32 = 4;
const CMD_PWM: u32 = 5;
const CMD_PRS: u32 = 6;
const CMD_PFS: u32 = 7;
const CMD_SERVO: u32 = 8;
const CMD_PIGPV: u32 = 26;

pub const INPUT: u32 = 0;
pub const OUTPUT: u32 = 1;

pub struct Pigpiod {
    stream: TcpStream,
}

impl Pigpiod {
    pub fn connect(addr: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        Ok(Pigpiod { stream })
    }

    // Sends one command and returns its result; negative results are pigpio error codes (see pigpio.h).
    fn command(&mut self, cmd: u32, p1: u32, p2: u32) -> io::Result<u32> {
        let mut buf = [0u8; 16];
        for (i, v) in [cmd, p1, p2, 0].iter().enumerate() {
            buf[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        self.stream.write_all(&buf)?;
        self.stream.read_exact(&mut buf)?;

        let res = i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        if res < 0 && cmd != CMD_PIGPV {
            return Err(io::Error::other(format!("pigpiod command {} ({}, {}) failed with error {}", cmd, p1, p2, res)));
        }
        Ok(res as u32)
    }

    pub fn version(&mut self) -> io::Result<u32> {
        self.command(CMD_PIGPV, 0, 0)
    }

    pub fn set_mode(&mut self, gpio: u32, mode: u32) -> io::Result<()> {
        self.command(CMD_MODES, gpio, mode).map(|_| ())
    }

    pub fn write(&mut self, gpio: u32, level: bool) -> io::Result<()> {
        self.command(CMD_WRITE, gpio, level as u32).map(|_| ())
    }

    // Returns the frequency actually selected, pigpio rounds to the closest available one.
    pub fn set_pwm_frequency(&mut self, gpio: u32, frequency: u32) -> io::Result<u32> {
        self.command(CMD_PFS, gpio, frequency)
    }

    // Returns the real range, i.e. the number of steps the hardware has at the current frequency.
    pub fn set_pwm_range(&mut self, gpio: u32, range: u32) -> io::Result<u32> {
        self.command(CMD_PRS, gpio, range)
    }

    pub fn pwm(&mut self, gpio: u32, dutycycle: u32) -> io::Result<()> {
        self.command(CMD_PWM, gpio, dutycycle).map(|_| ())
    }

    // Servo pulses at 50Hz, pulsewidth in us (500..2500), 0 switches the pulses off.
    pub fn servo(&mut self, gpio: u32, pulsewidth: u32) -> io::Result<()> {
        self.command(CMD_SERVO, gpio, pulsewidth).map(|_| ())
    }
}
