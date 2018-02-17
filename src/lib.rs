#![feature(asm)]

#![no_std]
#![crate_name = "onewire"]


extern crate embedded_hal as hal;

use hal::digital::OutputPin;
use hal::blocking::delay::DelayUs;

#[derive(Debug)]
pub enum OneWireError {
    WireNotHigh,
}

#[derive(Clone, PartialOrd, PartialEq)]
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

    pub fn search_next(&mut self, search: &mut OneWireDeviceSearch, delay: &mut DelayUs<u16>) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, delay, 0xF0)
    }

    pub fn search_next_alarmed(&mut self, search: &mut OneWireDeviceSearch, delay: &mut DelayUs<u16>) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, delay, 0xEC)
    }

    /// Heavily inspired by https://github.com/ntruchsess/arduino-OneWire/blob/85d1aae63ea4919c64151e03f7e24c2efbc40198/OneWire.cpp#L362
    fn search(&mut self, rom: &mut OneWireDeviceSearch, delay: &mut DelayUs<u16>, command: u8) -> Result<Option<OneWireDevice>, OneWireError> {
        let mut id_bit_number = 1_u8;
        let mut last_zero = 0_u8;
        let mut rom_byte_number = 0_usize;
        let mut search_result = false;

        let mut rom_byte_mask = 0_u8;
        let mut search_direction = false;


        if !rom.last_device_flag {
            if !self.reset(delay)? {
                return Ok(None);
            }

            self.write_byte(delay, command, false);

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
                            search_direction = rom.address[rom_byte_number] & rom_byte_mask > 0;
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
            byte <<= 1;
            if self.read_bit(delay) {
                byte |= 0x01;
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

    pub fn write(&mut self, delay: &mut DelayUs<u16>, bytes: &[u8]) {
        for b in bytes {
            self.write_byte(delay, *b, false);
        }
        if !self.parasite_mode {
            self.disable_parasite_mode();
        }
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

    #[inline]
    fn set_input(&mut self) {
        self.output.set_high()
    }

    #[inline]
    fn set_output(&mut self) {
        // nothing to do?
    }

    #[inline]
    fn write_low(&mut self) {
        self.output.set_low()
    }

    #[inline]
    fn write_high(&mut self) {
        self.output.set_high()
    }

    #[inline]
    fn read(&self) -> bool {
        self.output.is_high()
    }
}