
use byteorder::ByteOrder;
use byteorder::LittleEndian;
use hal::blocking::delay::DelayUs;

use OneWire;
use OneWireError;
use OneWireDevice;

#[repr(u8)]
pub enum Command {
    Convert = 0x44,
    WriteScratchpad = 0x4e,
    ReadScratchpad = 0xBE,
    CopyScratchpad = 0x48,
    RecallE2 = 0xB8,
    ReadPowerSupply = 0xB4,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum MeasureResolution {
    TC8 = 0b0001_1111,
    TC4 = 0b0011_1111,
    TC2 = 0b0101_1111,
    TC  = 0b0111_1111,
}

impl MeasureResolution {
    pub fn time_ms(&self) -> u16 {
        match self {
            &MeasureResolution::TC8 => 94,
            &MeasureResolution::TC4 => 188,
            &MeasureResolution::TC2 => 375,
            &MeasureResolution::TC  => 750,
        }
    }
}

pub struct DS18B20 {
    device: OneWireDevice,
    resolution: MeasureResolution,
}

impl DS18B20 {
    pub fn new(device: OneWireDevice) -> DS18B20 {
        DS18B20 {
            device,
            resolution: MeasureResolution::TC,
        }
    }

    pub fn measure_temperature(&self, wire: &mut OneWire, delay: &mut DelayUs<u16>) -> Result<MeasureResolution, OneWireError> {
        wire.reset_select_write_only(delay, &self.device, &[Command::Convert as u8])?;
        Ok(self.resolution)
    }

    pub fn read_temperature(&self, wire: &mut OneWire, delay: &mut DelayUs<u16>) -> Result<f32, OneWireError> {
        let mut scratchpad = [0u8; 9];
        wire.reset_select_write_read(delay, &self.device, &[Command::ReadScratchpad as u8], &mut scratchpad[..])?;
        OneWire::ensure_correct_rcr8(&self.device,&scratchpad[..8], scratchpad[8])?;
        Ok(DS18B20::read_temperature_from_scratchpad(&scratchpad))
    }

    fn read_temperature_from_scratchpad(scratchpad: &[u8]) -> f32 {
        let temp_u16 = LittleEndian::read_u16(&scratchpad[0..2]);
        let temp_f32 = temp_u16 as i16 as f32 / 16_f32;
        temp_f32
    }
}