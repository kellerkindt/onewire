#![no_std]
#![crate_name = "onewire"]

extern crate byteorder;
extern crate embedded_hal as hal;

pub mod ds18b20;

pub use crate::ds18b20::DS18B20;

use core::fmt::Formatter;
use core::fmt::{Debug, Display};
use hal::delay::DelayNs;
use hal::digital::InputPin;
use hal::digital::OutputPin;

pub const ADDRESS_BYTES: u8 = 8;
pub const ADDRESS_BITS: u8 = ADDRESS_BYTES * 8;

#[repr(u8)]
pub enum Command {
    SelectRom = 0x55,
    SearchNext = 0xF0,
    SearchNextAlarmed = 0xEC,
}

#[derive(Debug)]
pub enum Error<E: Sized + Debug> {
    WireNotHigh,
    CrcMismatch { computed: u8, expected: u8 },
    FamilyCodeMismatch { expected: u8, actual: u8 },
    Debug(Option<u8>),
    PortError(E),
}

impl<E: Sized + Debug> From<E> for Error<E> {
    fn from(e: E) -> Self {
        Error::PortError(e)
    }
}

#[cfg(feature = "defmt")]
impl<E: Sized + Debug> defmt::Format for Error<E> {
    fn format(&self, fmt: defmt::Formatter) {
        use defmt::write;
        match self {
            Error::WireNotHigh => write!(fmt, "WireNotHigh"),
            Error::CrcMismatch { expected, computed } => write!(
                fmt,
                "CrcMismatch {{ expected: {:04x}, computed: {:04x} }}",
                expected, computed
            ),
            Error::FamilyCodeMismatch { expected, actual } => write!(
                fmt,
                "FamilyCodeMismatch {{ expected: {:04x}, actual: {:04x} }}",
                expected, actual
            ),
            Error::Debug(value) => write!(fmt, "Debug {{ value: {:#04x} }}", value),
            #[cfg(feature = "defmt-debug2format")]
            Error::PortError(e) => {
                defmt::write!(fmt, "PortError {{ e: {:?} }}", defmt::Debug2Format(e))
            }
            #[cfg(not(feature = "defmt-debug2format"))]
            Error::PortError(_) => defmt::write!(
                fmt,
                "PortError {{ <enable onewire:defmt-debug2format to see more> }}"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub struct Device {
    pub address: [u8; ADDRESS_BYTES as usize],
}

#[cfg(feature = "defmt")]
impl defmt::Format for Device {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "Device {{ address: {=[u8; 8]:#04x} }}", self.address);
    }
}

impl Device {
    pub fn family_code(&self) -> u8 {
        self.address[0]
    }
}

impl core::str::FromStr for Device {
    type Err = core::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 23 {
            let _ = u8::from_str_radix("", 16)?; // this causes a ParseIntError::Empty
        }
        Ok(Device {
            address: [
                u8::from_str_radix(&s[0..2], 16)?,
                u8::from_str_radix(&s[3..5], 16)?,
                u8::from_str_radix(&s[6..8], 16)?,
                u8::from_str_radix(&s[9..11], 16)?,
                u8::from_str_radix(&s[12..14], 16)?,
                u8::from_str_radix(&s[15..17], 16)?,
                u8::from_str_radix(&s[18..20], 16)?,
                u8::from_str_radix(&s[21..23], 16)?,
            ],
        })
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum SearchState {
    #[default]
    Initialized,
    DeviceFound,
    End,
}

#[derive(Clone, Default)]
pub struct DeviceSearch {
    address: [u8; 8],
    discrepancies: [u8; 8],
    state: SearchState,
}

impl DeviceSearch {
    pub fn new() -> DeviceSearch {
        DeviceSearch::default()
    }

    pub fn new_for_family(family: u8) -> DeviceSearch {
        let mut search = DeviceSearch::new();
        search.address[0] = family;
        search
    }

    fn is_bit_set_in_address(&self, bit: u8) -> bool {
        DeviceSearch::is_bit_set(&self.address, bit)
    }

    fn set_bit_in_address(&mut self, bit: u8) {
        DeviceSearch::set_bit(&mut self.address, bit);
    }

    fn reset_bit_in_address(&mut self, bit: u8) {
        DeviceSearch::reset_bit(&mut self.address, bit);
    }

    fn write_bit_in_address(&mut self, bit: u8, value: bool) {
        if value {
            self.set_bit_in_address(bit);
        } else {
            self.reset_bit_in_address(bit);
        }
    }

    fn is_bit_set_in_discrepancies(&self, bit: u8) -> bool {
        DeviceSearch::is_bit_set(&self.discrepancies, bit)
    }

    fn set_bit_in_discrepancy(&mut self, bit: u8) {
        DeviceSearch::set_bit(&mut self.discrepancies, bit);
    }

    fn reset_bit_in_discrepancy(&mut self, bit: u8) {
        DeviceSearch::reset_bit(&mut self.discrepancies, bit);
    }

    #[allow(unused)] // useful method anyway?
    fn write_bit_in_discrepancy(&mut self, bit: u8, value: bool) {
        if value {
            self.set_bit_in_discrepancy(bit);
        } else {
            self.reset_bit_in_discrepancy(bit);
        }
    }

    fn is_bit_set(array: &[u8], bit: u8) -> bool {
        if bit / 8 >= array.len() as u8 {
            return false;
        }
        let index = bit / 8;
        let offset = bit % 8;
        array[index as usize] & (0x01 << offset) != 0x00
    }

    fn set_bit(array: &mut [u8], bit: u8) {
        if bit / 8 >= array.len() as u8 {
            return;
        }
        let index = bit / 8;
        let offset = bit % 8;
        array[index as usize] |= 0x01 << offset
    }

    fn reset_bit(array: &mut [u8], bit: u8) {
        if bit / 8 >= array.len() as u8 {
            return;
        }
        let index = bit / 8;
        let offset = bit % 8;
        array[index as usize] &= !(0x01 << offset)
    }

    pub fn last_discrepancy(&self) -> Option<u8> {
        let mut result = None;
        for i in 0..ADDRESS_BITS {
            if self.is_bit_set_in_discrepancies(i) {
                result = Some(i);
            }
        }
        result
    }

    pub fn into_iter<'a, ODO: OpenDrainOutput>(
        self,
        wire: &'a mut OneWire<ODO>,
        delay: &'a mut impl DelayNs,
    ) -> DeviceSearchIter<'a, ODO, impl DelayNs> {
        DeviceSearchIter {
            search: Some(self),
            wire,
            delay,
        }
    }
}

pub struct DeviceSearchIter<'a, ODO: OpenDrainOutput, Delay: DelayNs> {
    search: Option<DeviceSearch>,
    wire: &'a mut OneWire<ODO>,
    delay: &'a mut Delay,
}

impl<ODO: OpenDrainOutput, Delay: DelayNs> Iterator for DeviceSearchIter<'_, ODO, Delay> {
    type Item = Result<Device, Error<ODO::Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut search = self.search.take()?;
        let result = self
            .wire
            .search_next(&mut search, &mut *self.delay)
            .transpose()?;
        self.search = Some(search);
        Some(result)
    }
}

pub trait OpenDrainOutput {
    type Error: Sized + Debug;

    /// Is the input pin high?
    fn is_high(&mut self) -> Result<bool, Self::Error>;

    /// Is the input pin low?
    fn is_low(&mut self) -> Result<bool, Self::Error>;

    /// Drives the pin low
    ///
    /// *NOTE* the actual electrical state of the pin may not actually be low, e.g. due to external
    /// electrical sources
    fn set_low(&mut self) -> Result<(), Self::Error>;

    /// Drives the pin high
    ///
    /// *NOTE* the actual electrical state of the pin may not actually be high, e.g. due to external
    /// electrical sources
    fn set_high(&mut self) -> Result<(), Self::Error>;
}
impl<E: Debug, P: OutputPin<Error = E> + InputPin<Error = E>> OpenDrainOutput for P {
    type Error = E;

    fn is_high(&mut self) -> Result<bool, Self::Error> {
        InputPin::is_high(self)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        InputPin::is_low(self)
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        OutputPin::set_low(self)
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        OutputPin::set_high(self)
    }
}

pub struct OneWire<ODO: OpenDrainOutput> {
    output: ODO,
    parasite_mode: bool,
}

impl<E: core::fmt::Debug, ODO: OpenDrainOutput<Error = E>> OneWire<ODO> {
    pub fn new(output: ODO, parasite_mode: bool) -> Self {
        OneWire {
            output,
            parasite_mode,
        }
    }

    pub fn reset_select_write_read(
        &mut self,
        delay: &mut impl DelayNs,
        device: &Device,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Error<E>> {
        self.reset(delay)?;
        self.select(delay, device)?;
        self.write_bytes(delay, write)?;
        self.read_bytes(delay, read)?;
        Ok(())
    }

    pub fn reset_select_read_only(
        &mut self,
        delay: &mut impl DelayNs,
        device: &Device,
        read: &mut [u8],
    ) -> Result<(), Error<E>> {
        self.reset(delay)?;
        self.select(delay, device)?;
        self.read_bytes(delay, read)?;
        Ok(())
    }

    pub fn reset_select_write_only(
        &mut self,
        delay: &mut impl DelayNs,
        device: &Device,
        write: &[u8],
    ) -> Result<(), Error<E>> {
        self.reset(delay)?;
        self.select(delay, device)?;
        self.write_bytes(delay, write)?;
        Ok(())
    }

    pub fn select(&mut self, delay: &mut impl DelayNs, device: &Device) -> Result<(), Error<E>> {
        let parasite_mode = self.parasite_mode;
        self.write_command(delay, Command::SelectRom, parasite_mode)?; // select
        for i in 0..device.address.len() {
            let last = i == device.address.len() - 1;
            self.write_byte(delay, device.address[i], parasite_mode && last)?;
        }
        Ok(())
    }

    pub fn search_next(
        &mut self,
        search: &mut DeviceSearch,
        delay: &mut impl DelayNs,
    ) -> Result<Option<Device>, Error<E>> {
        self.search(search, delay, Command::SearchNext)
    }

    pub fn search_next_alarmed(
        &mut self,
        search: &mut DeviceSearch,
        delay: &mut impl DelayNs,
    ) -> Result<Option<Device>, Error<E>> {
        self.search(search, delay, Command::SearchNextAlarmed)
    }

    /// Heavily inspired by https://github.com/ntruchsess/arduino-OneWire/blob/85d1aae63ea4919c64151e03f7e24c2efbc40198/OneWire.cpp#L362
    fn search(
        &mut self,
        rom: &mut DeviceSearch,
        delay: &mut impl DelayNs,
        cmd: Command,
    ) -> Result<Option<Device>, Error<E>> {
        if SearchState::End == rom.state {
            return Ok(None);
        }

        let mut discrepancy_found = false;
        let last_discrepancy = rom.last_discrepancy();

        if !self.reset(delay)? {
            return Ok(None);
        }

        self.write_byte(delay, cmd as u8, false)?;

        if let Some(last_discrepancy) = last_discrepancy {
            // walk previous path
            for i in 0..last_discrepancy {
                let bit0 = self.read_bit(delay)?;
                let bit1 = self.read_bit(delay)?;

                if bit0 && bit1 {
                    // no device responded
                    return Ok(None);
                } else {
                    let bit = rom.is_bit_set_in_address(i);
                    // rom.write_bit_in_address(i, bit0);
                    // rom.write_bit_in_discrepancy(i, bit);
                    self.write_bit(delay, bit)?;
                }
            }
        } else {
            // no discrepancy and device found, meaning the one found is the only one
            if rom.state == SearchState::DeviceFound {
                rom.state = SearchState::End;
                return Ok(None);
            }
        }

        for i in last_discrepancy.unwrap_or(0)..ADDRESS_BITS {
            let bit0 = self.read_bit(delay)?; // normal bit
            let bit1 = self.read_bit(delay)?; // complementar bit

            if last_discrepancy.eq(&Some(i)) {
                // be sure to go different path from before (go second path, thus writing 1)
                rom.reset_bit_in_discrepancy(i);
                rom.set_bit_in_address(i);
                self.write_bit(delay, true)?;
            } else {
                if bit0 && bit1 {
                    // no response received
                    return Ok(None);
                }

                if !bit0 && !bit1 {
                    // addresses with 0 and 1
                    // found new path, go first path by default (thus writing 0)
                    discrepancy_found |= true;
                    rom.set_bit_in_discrepancy(i);
                    rom.reset_bit_in_address(i);
                    self.write_bit(delay, false)?;
                } else {
                    // addresses only with bit0
                    rom.write_bit_in_address(i, bit0);
                    self.write_bit(delay, bit0)?;
                }
            }
        }

        if !discrepancy_found && rom.last_discrepancy().is_none() {
            rom.state = SearchState::End;
        } else {
            rom.state = SearchState::DeviceFound;
        }
        Ok(Some(Device {
            address: rom.address,
        }))
    }

    /// Performs a reset and listens for a presence pulse
    /// Returns Err(WireNotHigh) if the wire seems to be shortened,
    /// Ok(true) if presence pulse has been received and Ok(false)
    /// if no other device was detected but the wire seems to be ok
    pub fn reset(&mut self, delay: &mut impl DelayNs) -> Result<bool, Error<E>> {
        // let mut cli = DisableInterrupts::new();
        self.set_input()?;
        // drop(cli);

        self.ensure_wire_high(delay)?;
        // cli = DisableInterrupts::new();
        self.write_low()?;
        self.set_output()?;

        // drop(cli);
        delay.delay_us(480);
        // cli = DisableInterrupts::new();
        self.set_input()?;

        let mut val = false;
        for _ in 0..7 {
            delay.delay_us(10);
            val |= !self.read()?;
        }
        // drop(cli);
        delay.delay_us(410);
        Ok(val)
    }

    fn ensure_wire_high(&mut self, delay: &mut impl DelayNs) -> Result<(), Error<E>> {
        for _ in 0..125 {
            if self.read()? {
                return Ok(());
            }
            delay.delay_us(2);
        }
        Err(Error::WireNotHigh)
    }

    pub fn read_bytes(&mut self, delay: &mut impl DelayNs, dst: &mut [u8]) -> Result<(), E> {
        for d in dst {
            *d = self.read_byte(delay)?;
        }
        Ok(())
    }

    fn read_byte(&mut self, delay: &mut impl DelayNs) -> Result<u8, E> {
        let mut byte = 0_u8;
        for _ in 0..8 {
            byte >>= 1;
            if self.read_bit(delay)? {
                byte |= 0x80;
            }
        }
        Ok(byte)
    }

    fn read_bit(&mut self, delay: &mut impl DelayNs) -> Result<bool, E> {
        // let cli = DisableInterrupts::new();
        self.set_output()?;
        self.write_low()?;
        delay.delay_us(3);
        self.set_input()?;
        delay.delay_us(2); // was 10
        let val = self.read();
        // drop(cli);
        delay.delay_us(61); // was 53
        val
    }

    pub fn write_bytes(&mut self, delay: &mut impl DelayNs, bytes: &[u8]) -> Result<(), E> {
        for b in bytes {
            self.write_byte(delay, *b, false)?;
        }
        if !self.parasite_mode {
            self.disable_parasite_mode()?;
        }
        Ok(())
    }

    fn write_command(
        &mut self,
        delay: &mut impl DelayNs,
        cmd: Command,
        parasite_mode: bool,
    ) -> Result<(), E> {
        self.write_byte(delay, cmd as u8, parasite_mode)
    }

    fn write_byte(
        &mut self,
        delay: &mut impl DelayNs,
        mut byte: u8,
        parasite_mode: bool,
    ) -> Result<(), E> {
        for _ in 0..8 {
            self.write_bit(delay, (byte & 0x01) == 0x01)?;
            byte >>= 1;
        }
        if !parasite_mode {
            self.disable_parasite_mode()?;
        }
        Ok(())
    }

    fn write_bit(&mut self, delay: &mut impl DelayNs, high: bool) -> Result<(), E> {
        // let cli = DisableInterrupts::new();
        self.write_low()?;
        self.set_output()?;
        delay.delay_us(if high { 10 } else { 65 });
        self.write_high()?;
        // drop(cli);
        delay.delay_us(if high { 55 } else { 5 });
        Ok(())
    }

    fn disable_parasite_mode(&mut self) -> Result<(), E> {
        // let cli = DisableInterrupts::new();
        self.set_input()?;
        self.write_low()
    }

    fn set_input(&mut self) -> Result<(), E> {
        self.output.set_high()
    }

    fn set_output(&mut self) -> Result<(), E> {
        // nothing to do?
        Ok(())
    }

    fn write_low(&mut self) -> Result<(), E> {
        self.output.set_low()
    }

    fn write_high(&mut self) -> Result<(), E> {
        self.output.set_high()
    }

    fn read(&mut self) -> Result<bool, E> {
        self.output.is_high()
    }
}

pub fn ensure_correct_rcr8<E: Debug>(
    device: &Device,
    data: &[u8],
    crc8: u8,
) -> Result<(), Error<E>> {
    let computed = compute_crc8(device, data);
    if computed != crc8 {
        Err(Error::CrcMismatch {
            computed,
            expected: crc8,
        })
    } else {
        Ok(())
    }
}

const CRC_TABLE: [u8; 256] = [
    0x00, 0x5E, 0xBC, 0xE2, 0x61, 0x3F, 0xDD, 0x83, 0xC2, 0x9C, 0x7E, 0x20, 0xA3,
    0xFD, 0x1F, 0x41, 0x9D, 0xC3, 0x21, 0x7F, 0xFC, 0xA2, 0x40, 0x1E, 0x5F, 0x01,
    0xE3, 0xBD, 0x3E, 0x60, 0x82, 0xDC, 0x23, 0x7D, 0x9F, 0xC1, 0x42, 0x1C, 0xFE,
    0xA0, 0xE1, 0xBF, 0x5D, 0x03, 0x80, 0xDE, 0x3C, 0x62, 0xBE, 0xE0, 0x02, 0x5C,
    0xDF, 0x81, 0x63, 0x3D, 0x7C, 0x22, 0xC0, 0x9E, 0x1D, 0x43, 0xA1, 0xFF, 0x46,
    0x18, 0xFA, 0xA4, 0x27, 0x79, 0x9B, 0xC5, 0x84, 0xDA, 0x38, 0x66, 0xE5, 0xBB,
    0x59, 0x07, 0xDB, 0x85, 0x67, 0x39, 0xBA, 0xE4, 0x06, 0x58, 0x19, 0x47, 0xA5,
    0xFB, 0x78, 0x26, 0xC4, 0x9A, 0x65, 0x3B, 0xD9, 0x87, 0x04, 0x5A, 0xB8, 0xE6,
    0xA7, 0xF9, 0x1B, 0x45, 0xC6, 0x98, 0x7A, 0x24, 0xF8, 0xA6, 0x44, 0x1A, 0x99,
    0xC7, 0x25, 0x7B, 0x3A, 0x64, 0x86, 0xD8, 0x5B, 0x05, 0xE7, 0xB9, 0x8C, 0xD2,
    0x30, 0x6E, 0xED, 0xB3, 0x51, 0x0F, 0x4E, 0x10, 0xF2, 0xAC, 0x2F, 0x71, 0x93,
    0xCD, 0x11, 0x4F, 0xAD, 0xF3, 0x70, 0x2E, 0xCC, 0x92, 0xD3, 0x8D, 0x6F, 0x31,
    0xB2, 0xEC, 0x0E, 0x50, 0xAF, 0xF1, 0x13, 0x4D, 0xCE, 0x90, 0x72, 0x2C, 0x6D,
    0x33, 0xD1, 0x8F, 0x0C, 0x52, 0xB0, 0xEE, 0x32, 0x6C, 0x8E, 0xD0, 0x53, 0x0D,
    0xEF, 0xB1, 0xF0, 0xAE, 0x4C, 0x12, 0x91, 0xCF, 0x2D, 0x73, 0xCA, 0x94, 0x76,
    0x28, 0xAB, 0xF5, 0x17, 0x49, 0x08, 0x56, 0xB4, 0xEA, 0x69, 0x37, 0xD5, 0x8B,
    0x57, 0x09, 0xEB, 0xB5, 0x36, 0x68, 0x8A, 0xD4, 0x95, 0xCB, 0x29, 0x77, 0xF4,
    0xAA, 0x48, 0x16, 0xE9, 0xB7, 0x55, 0x0B, 0x88, 0xD6, 0x34, 0x6A, 0x2B, 0x75,
    0x97, 0xC9, 0x4A, 0x14, 0xF6, 0xA8, 0x74, 0x2A, 0xC8, 0x96, 0x15, 0x4B, 0xA9,
    0xF7, 0xB6, 0xE8, 0x0A, 0x54, 0xD7, 0x89, 0x6B, 0x35,
];

pub fn compute_crc8(device: &Device, data: &[u8]) -> u8 {
    device.address.iter().chain(data).fold(0u8, |acc, &byte| {
        CRC_TABLE[(byte ^ acc) as usize]
    })
}

impl Display for Device {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.address[0],
            self.address[1],
            self.address[2],
            self.address[3],
            self.address[4],
            self.address[5],
            self.address[6],
            self.address[7],
        )
    }
}

pub trait Sensor {
    fn family_code() -> u8;

    /// returns the milliseconds required to wait until the measurement finished
    fn start_measurement<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<u16, Error<O::Error>>;

    /// returns the measured value
    fn read_measurement<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<f32, Error<O::Error>>;

    fn read_measurement_raw<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<u16, Error<O::Error>>;
}
