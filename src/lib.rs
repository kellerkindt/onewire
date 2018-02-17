#![feature(asm)]

#![no_std]
#![crate_name = "onewire"]




pub trait OpenDrainOutput {

    fn drain_low(&mut self);

    fn float_high(&mut self);

    fn is_low(&self) -> bool {
        !self.is_high()
    }

    fn is_high(&self) -> bool;

    fn delay_us(&mut self, us: u16);
}


#[derive(Debug)]
pub enum OneWireError {
    WireNotHigh,
}

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

pub struct OneWire<O: OpenDrainOutput + Sized> {
    output: O,
    parasite_mode: bool,
}

impl<O: OpenDrainOutput + Sized> OneWire<O> {

    pub fn new(output: O, parasite_mode: bool) -> OneWire<O> {
        OneWire {
            output,
            parasite_mode,
        }
    }

    pub fn search_next(&mut self, search: &mut OneWireDeviceSearch) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, 0xF0)
    }

    pub fn search_next_alarmed(&mut self, search: &mut OneWireDeviceSearch) -> Result<Option<OneWireDevice>, OneWireError> {
        self.search(search, 0xEC)
    }

    /// Heavily inspired by https://github.com/ntruchsess/arduino-OneWire/blob/85d1aae63ea4919c64151e03f7e24c2efbc40198/OneWire.cpp#L362
    fn search(&mut self, rom: &mut OneWireDeviceSearch, command: u8) -> Result<Option<OneWireDevice>, OneWireError> {
        let mut id_bit_number = 1_u8;
        let mut last_zero = 0_u8;
        let mut rom_byte_number = 0_usize;
        let mut search_result = false;

        let mut rom_byte_mask = 0_u8;
        let mut search_direction = false;


        if !rom.last_device_flag {
            if !self.reset()? {
                return Ok(None);
            }

            self.write_byte(command, false);

            while rom_byte_number < 8 {
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
    pub fn reset(&mut self) -> Result<bool, OneWireError> {
        // let mut cli = DisableInterrupts::new();
        self.set_input();
        // drop(cli);

        self.ensure_wire_high()?;
        // cli = DisableInterrupts::new();
        self.write_low();
        self.set_output();

        // drop(cli);
        self.delay_us(480);
        // cli = DisableInterrupts::new();
        self.set_input();

        let mut val = false;
        for _ in 0..7 {
            self.delay_us(10);
            val |= !self.read();
        }
        // drop(cli);
        self.delay_us(410);
        Ok(val)
    }

    fn ensure_wire_high(&mut self) -> Result<(), OneWireError> {
        for _ in 0..125 {
            if self.read() {
                return Ok(());
            }
            self.delay_us(2);
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

    fn read_bit(&mut self) -> bool {
        // let cli = DisableInterrupts::new();
        self.set_output();
        self.write_low();
        self.delay_us(3);
        self.set_input();
        self.delay_us(10);
        let val = self.read();
        // drop(cli);
        self.delay_us(53);
        val
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_byte(*b, false);
        }
        if !self.parasite_mode {
            self.disable_parasite_mode();
        }
    }

    fn write_byte(&mut self, mut byte: u8, parasite_mode: bool) {
        for _ in 0..8 {
            self.write_bit((byte & 0x01) == 0x01);
            byte >>= 1;
        }
        if !parasite_mode {
            self.disable_parasite_mode();
        }
    }

    fn write_bit(&mut self, high: bool) {
        // let cli = DisableInterrupts::new();
        self.write_low();
        self.set_output();
        self.delay_us(if high {10} else {65});
        self.write_high();
        // drop(cli);
        self.delay_us(if high {55} else {5})
    }


    fn disable_parasite_mode(&mut self) {
        // let cli = DisableInterrupts::new();
        self.set_input();
        self.write_low();
    }

    fn set_input(&mut self) {
        self.output.float_high()
    }

    fn set_output(&mut self) {
        // nothing to do?
    }

    fn write_low(&mut self) {
        self.output.drain_low()
    }

    fn write_high(&mut self) {
        self.output.float_high()
    }

    fn read(&self) -> bool {
        self.output.is_high()
    }
    
    fn delay_us(&mut self, us: u16) {
        self.output.delay_us(us)
    }
}