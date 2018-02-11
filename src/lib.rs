#![feature(asm)]

#![no_std]

#![crate_name = "onewire"]

extern crate arduino;
extern crate avr_delay;

use avr_delay::delay_us;
use arduino::prelude::DisableInterrupts;

pub enum OneWireError {
    WireNotHigh,
}

#[derive(Default)]
pub struct OneWireDevice {
    pub address: [u8; 8]
}

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

pub struct OneWire {
    pin: *mut u8, // PIN+1 => DDR+1 => PORT
    mask: u8,
    parasite_mode: bool,
}

impl OneWire {

    pub fn new(pin: *mut u8, pin_no: u8, parasite_mode: bool) -> OneWire {
        OneWire {
            pin,
            mask: 0x01 << pin_no,
            parasite_mode,
        }
    }

    pub fn search_next(&self, search: &mut OneWireDeviceSearch) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, 0xF0)
    }

    pub fn search_next_alarmed(&self, search: &mut OneWireDeviceSearch) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, 0xEC)
    }

    /// Heavily inspired by https://github.com/ntruchsess/arduino-OneWire/blob/85d1aae63ea4919c64151e03f7e24c2efbc40198/OneWire.cpp#L362
    fn search(&self, rom: &mut OneWireDeviceSearch, command: u8) -> Result<Option<OneWireDevice>, OneWireError> {
        let mut id_bit_number = 1_u8;
        let mut last_zero = 0_u8;
        let mut rom_byte_number = 0_usize;
        let mut rom_byte_mask = 0_u8;
        let mut search_result = false;

        let mut rom_byte_mask = 0_u8;
        let mut search_direction = false;

        if !rom.last_device_flag {
            if !self.reset()? {
                return Ok(None);
            }

            self.write_byte(command, false);

            loop {
                let id_bit = self.read_bit();
                let cmp_id_bit = self.read_bit();

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

                self.write_bit(search_direction);

                id_bit_number += 1;
                rom_byte_mask <<= 1;

                if rom_byte_mask == 0 {
                    rom_byte_number += 1;
                    rom_byte_mask = 0x01;
                }

                if rom_byte_number >= 8 {
                    break;
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
    pub fn reset(&self) -> Result<bool, OneWireError> {
        unsafe {
            self.set_input_mode();
        }

        self.ensure_wire_high()?;
        let mut cli = DisableInterrupts::new();
        unsafe {
            self.write_low();
            self.set_output_mode();
        }
        drop(cli);
        delay_us(480);
        unsafe {
            cli = DisableInterrupts::new();
            self.set_input_mode();
        }
        delay_us(70);
        let val = unsafe {
            self.read()
        };
        drop(cli);
        delay_us(410);
        Ok(!val)
    }

    fn ensure_wire_high(&self) -> Result<(), OneWireError> {
        for _ in 0..125 {
            unsafe {
                if self.read() {
                    return Ok(());
                }
            }
            delay_us(2);
        }
        Err(OneWireError::WireNotHigh)
    }

    pub fn read_bytes(&self, dst: &mut [u8]) {
        for i in 0..dst.len() {
            dst[i] = self.read_byte();
        }
    }

    fn read_byte(&self) -> u8 {
        let mut byte = 0_u8;
        for _ in 0..8 {
            byte <<= 1;
            byte |= self.read_byte();
        }
        byte
    }

    fn read_bit(&self) -> bool {
        let cli = DisableInterrupts::new();
        unsafe {
            self.set_output_mode();
            self.write_low();
            delay_us(3);
            self.set_input_mode();
            delay_us(10);
            let val = self.read();
            drop(cli);
            delay_us(53);
            val
        }
    }

    pub fn write(&self, bytes: &[u8]) {
        for b in bytes {
            self.write_byte(*b, false);
        }
        if !self.parasite_mode {
            self.disable_parasite_mode();
        }
    }

    fn write_byte(&self, mut byte: u8, parasite_mode: bool) {
        for _ in 0..8 {
            self.write_bit((byte & 0x01) == 0x01);
            byte >>= 1;
        }
        if !parasite_mode {
            self.disable_parasite_mode();
        }
    }

    fn write_bit(&self, bit: bool) {
        let cli = DisableInterrupts::new();
        unsafe {
            self.write_low();
            self.set_output_mode();
            delay_us(if bit {10} else {65});
            self.write_high();
            drop(cli);
            delay_us(if bit {55} else {5})
        }
    }


    fn disable_parasite_mode(&self) {
        let cli = DisableInterrupts::new();
        unsafe {
            self.set_input_mode();
            self.write_low();
        }
    }

    #[inline]
    fn pin(&self) -> *mut u8 {
        self.pin
    }

    #[inline]
    fn ddr(&self) -> *mut u8 {
        (self.pin as usize + 1_usize) as *mut u8
    }

    #[inline]
    fn port(&self) -> *mut u8 {
        (self.pin as usize + 2_usize) as *mut u8
    }

    #[inline]
    unsafe fn set_input_mode(&self) {
        *self.ddr() &= !self.mask
    }

    #[inline]
    unsafe fn set_output_mode(&self) {
        *self.ddr() |= self.mask
    }

    #[inline]
    unsafe fn write_low(&self) {
        *self.port() &= !self.mask
    }

    #[inline]
    unsafe fn write_high(&self) {
        *self.port() |= self.mask
    }

    #[inline]
    unsafe fn read(&self) -> bool {
        *self.pin() & self.mask == self.mask
    }
}