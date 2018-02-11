#![feature(asm)]

#![no_std]

#![crate_name = "onewire"]

extern crate arduino;
extern crate avr_delay;


use avr_delay::delay_us;
use core::ptr::write_volatile;
use core::ptr::read_volatile;
use arduino::prelude::DisableInterrupts;

pub enum OneWireError {
    WireNotHigh,
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
            byte |= if self.read_bit() {0x01} else {0x00};
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