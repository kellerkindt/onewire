#![feature(asm)]

#![no_std]
#![crate_name = "onewire"]

extern crate byteorder;
extern crate embedded_hal as hal;

pub mod ds18b20;

pub use ds18b20::DS18B20;

use hal::digital::OutputPin;
use hal::blocking::delay::DelayUs;

#[repr(u8)]
pub enum Command {
    SelectRom = 0x55,
    SearchNext = 0xF0,
    SearchNextAlarmed = 0xEC,
}

#[derive(Debug)]
pub enum OneWireError {
    WireNotHigh,
    CrcMismatch(u8, u8),
    FamilyCodeMismatch(u8, u8),
}

#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub struct OneWireDevice {
    pub address: [u8; 8]
}

#[derive(Clone)]
pub struct OneWireDeviceSearch {
    address: [u8; 8],
    last_discrepancy: u8,
    last_family_discrepancy: u8,
    last_device_flag: bool
}

impl OneWireDeviceSearch {
    pub fn new() -> OneWireDeviceSearch {
        OneWireDeviceSearch {
            address: [0u8; 8],
            last_discrepancy: 0u8,
            last_device_flag: false,
            last_family_discrepancy: 0,
        }
    }

    pub fn new_for_family(family: u8) -> OneWireDeviceSearch {
        let mut search = OneWireDeviceSearch::new();
        search.address[0] = family;
        search
    }
}

pub struct OneWire<'a> {
    output: &'a mut OutputPin,
    parasite_mode: bool,
}

impl<'a> OneWire<'a> {

    pub fn new(output: &'a mut OutputPin, parasite_mode: bool) -> OneWire<'a> {
        OneWire {
            output,
            parasite_mode,
        }
    }

    pub fn reset_select_write_read(&mut self, delay: &mut DelayUs<u16>, device: &OneWireDevice, write: &[u8], read: &mut [u8]) -> Result<(), OneWireError> {
        self.reset(delay)?;
        self.select(delay, device);
        self.write_bytes(delay, write);
        self.read_bytes(delay, read);
        Ok(())
    }

    pub fn reset_select_read_only(&mut self, delay: &mut DelayUs<u16>, device: &OneWireDevice, read: &mut [u8]) -> Result<(), OneWireError> {
        self.reset(delay)?;
        self.select(delay, device);
        self.read_bytes(delay, read);
        Ok(())
    }

    pub fn reset_select_write_only(&mut self, delay: &mut DelayUs<u16>, device: &OneWireDevice, write: &[u8]) -> Result<(), OneWireError> {
        self.reset(delay)?;
        self.select(delay, device);
        self.write_bytes(delay, write);
        Ok(())
    }

    pub fn select(&mut self, delay: &mut DelayUs<u16>, device: &OneWireDevice) {
        let parasite_mode = self.parasite_mode;
        self.write_command(delay, Command::SelectRom, parasite_mode); // select
        for i in 0..device.address.len() {
            let last = i == device.address.len() - 1;
            self.write_byte(delay, device.address[i], parasite_mode && last);
        }
    }

    pub fn search_next(&mut self, search: &mut OneWireDeviceSearch, delay: &mut DelayUs<u16>) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, delay, Command::SearchNext)
    }

    pub fn search_next_alarmed(&mut self, search: &mut OneWireDeviceSearch, delay: &mut DelayUs<u16>) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, delay, Command::SearchNextAlarmed)
    }

    /// Heavily inspired by https://github.com/ntruchsess/arduino-OneWire/blob/85d1aae63ea4919c64151e03f7e24c2efbc40198/OneWire.cpp#L362
    fn search(&mut self, rom: &mut OneWireDeviceSearch, delay: &mut DelayUs<u16>, cmd: Command) -> Result<Option<OneWireDevice>, OneWireError> {
        let mut id_bit_number = 1_u8;
        let mut last_zero = 0_u8;
        let mut rom_byte_number = 0_usize;
        let mut rom_byte_mask = 1_u8;
        let mut search_result = false;

        let mut search_direction : bool;

        if !rom.last_device_flag {
            if !self.reset(delay)? {
                return Ok(None);
            }

            self.write_byte(delay, cmd as u8, false);

            while rom_byte_number < 8 {
                let id_bit = self.read_bit(delay);
                let cmp_id_bit = self.read_bit(delay);

                // no device?
                if id_bit && cmp_id_bit {
                    break;
                } else {
                    // devices have 0 or 1
                    if id_bit != cmp_id_bit {
                        // bit write value for search
                        search_direction = id_bit;
                    } else {
                        if id_bit_number < rom.last_discrepancy {
                            search_direction = (rom.address[rom_byte_number] & rom_byte_mask) > 0;
                        } else {
                            search_direction = id_bit_number == rom.last_discrepancy;
                        }
                    }

                    if !search_direction {
                        last_zero = id_bit_number;

                        if last_zero < 9 {
                            rom.last_family_discrepancy = last_zero;
                        }
                    }
                }

                if search_direction {
                    rom.address[rom_byte_number] |= rom_byte_mask;
                } else {
                    rom.address[rom_byte_number] &= !rom_byte_mask;
                }

                self.write_bit(delay, search_direction);

                id_bit_number += 1;
                rom_byte_mask <<= 1;

                if rom_byte_mask == 0 {
                    rom_byte_number += 1;
                    rom_byte_mask = 0x01;
                }
            }

            if id_bit_number >= 65 {
                rom.last_discrepancy = last_zero;

                if rom.last_discrepancy == 0 {
                    rom.last_device_flag = true;
                }

                search_result = true;
            }
        }

        if !search_result || rom.address[0] == 0x00 {
            rom.last_discrepancy = 0;
            rom.last_device_flag = false;
            rom.last_family_discrepancy = 0;
            search_result = false;
        }

        if search_result {
            Ok(Some(OneWireDevice {
                address: rom.address
            }))
        } else {
            Ok(None)
        }
    }

    /// Performs a reset and listens for a presence pulse
    /// Returns Err(WireNotHigh) if the wire seems to be shortened,
    /// Ok(true) if presence pulse has been received and Ok(false)
    /// if no other device was detected but the wire seems to be ok
    pub fn reset(&mut self, delay: &mut DelayUs<u16>) -> Result<bool, OneWireError> {
        // let mut cli = DisableInterrupts::new();
        self.set_input();
        // drop(cli);

        self.ensure_wire_high(delay)?;
        // cli = DisableInterrupts::new();
        self.write_low();
        self.set_output();

        // drop(cli);
        delay.delay_us(480);
        // cli = DisableInterrupts::new();
        self.set_input();

        let mut val = false;
        for _ in 0..7 {
            delay.delay_us(10);
            val |= !self.read();
        }
        // drop(cli);
        delay.delay_us(410);
        Ok(val)
    }

    fn ensure_wire_high(&mut self, delay: &mut DelayUs<u16>) -> Result<(), OneWireError> {
        for _ in 0..125 {
            if self.read() {
                return Ok(());
            }
            delay.delay_us(2);
        }
        Err(OneWireError::WireNotHigh)
    }

    pub fn read_bytes(&mut self, delay: &mut DelayUs<u16>, dst: &mut [u8]) {
        for i in 0..dst.len() {
            dst[i] = self.read_byte(delay);
        }
    }

    fn read_byte(&mut self, delay: &mut DelayUs<u16>) -> u8 {
        let mut byte = 0_u8;
        for _ in 0..8 {
            byte >>= 1;
            if self.read_bit(delay) {
                byte |= 0x80;
            }
        }
        byte
    }

    fn read_bit(&mut self, delay: &mut DelayUs<u16>) -> bool {
        // let cli = DisableInterrupts::new();
        self.set_output();
        self.write_low();
        delay.delay_us(3);
        self.set_input();
        delay.delay_us(10);
        let val = self.read();
        // drop(cli);
        delay.delay_us(53);
        val
    }

    pub fn write_bytes(&mut self, delay: &mut DelayUs<u16>, bytes: &[u8]) {
        for b in bytes {
            self.write_byte(delay, *b, false);
        }
        if !self.parasite_mode {
            self.disable_parasite_mode();
        }
    }

    fn write_command(&mut self, delay: &mut DelayUs<u16>, cmd: Command, parasite_mode: bool) {
        self.write_byte(delay, cmd as u8, parasite_mode)
    }

    fn write_byte(&mut self, delay: &mut DelayUs<u16>, mut byte: u8, parasite_mode: bool) {
        for _ in 0..8 {
            self.write_bit(delay, (byte & 0x01) == 0x01);
            byte >>= 1;
        }
        if !parasite_mode {
            self.disable_parasite_mode();
        }
    }

    fn write_bit(&mut self, delay: &mut DelayUs<u16>, high: bool) {
        // let cli = DisableInterrupts::new();
        self.write_low();
        self.set_output();
        delay.delay_us(if high {10} else {65});
        self.write_high();
        // drop(cli);
        delay.delay_us(if high {55} else {5})
    }


    fn disable_parasite_mode(&mut self) {
        // let cli = DisableInterrupts::new();
        self.set_input();
        self.write_low();
    }

    fn set_input(&mut self) {
        self.output.set_high()
    }

    fn set_output(&mut self) {
        // nothing to do?
    }

    fn write_low(&mut self) {
        self.output.set_low()
    }

    fn write_high(&mut self) {
        self.output.set_high()
    }

    fn read(&self) -> bool {
        self.output.is_high()
    }

    pub fn ensure_correct_rcr8(device: &OneWireDevice, data: &[u8], crc8: u8) -> Result<(), OneWireError> {
        let computed = OneWire::compute_crc8(device, data);
        if computed != crc8 {
            Err(OneWireError::CrcMismatch(computed, crc8))
        } else {
            Ok(())
        }
    }

    pub fn compute_crc8(device: &OneWireDevice, data: &[u8]) -> u8 {
        let crc = OneWire::compute_partial_crc8(0u8, &device.address[..]);
        OneWire::compute_partial_crc8(crc, data)
    }

    pub fn compute_partial_crc8(crc: u8, data: &[u8]) -> u8 {
        let mut crc = crc;
        for byte in data.iter() {
            let mut byte = *byte;
            for _ in 0..8 {
                let mix = (crc ^ byte) & 0x01;
                crc >>= 1;
                if mix != 0x00 {
                    crc ^= 0x8C;
                }
                byte >>= 1;
            }
        }
        crc
    }
}

use core::fmt::Display;
use core::fmt::Formatter;

impl Display for OneWireDevice {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
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