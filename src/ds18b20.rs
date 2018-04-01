
use byteorder::ByteOrder;
use byteorder::LittleEndian;
use hal::blocking::delay::DelayUs;

use Error;
use Device;
use Sensor;
use OneWire;

pub const FAMILY_CODE : u8 = 0x28;

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
    device: Device,
    resolution: MeasureResolution,
}

impl DS18B20 {
    pub fn new(device: Device) -> Result<DS18B20, Error> {
        if device.address[0] != FAMILY_CODE {
            Err(Error::FamilyCodeMismatch(FAMILY_CODE, device.address[0]))
        } else {
            Ok(DS18B20 {
                device,
                resolution: MeasureResolution::TC,
            })
        }
    }

    pub unsafe fn new_forced(device: Device) -> DS18B20 {
        DS18B20 {
            device,
            resolution: MeasureResolution::TC
        }
    }

    pub fn measure_temperature(&self, wire: &mut OneWire, delay: &mut DelayUs<u16>) -> Result<MeasureResolution, Error> {
        wire.reset_select_write_only(delay, &self.device, &[Command::Convert as u8])?;
        Ok(self.resolution)
    }

    pub fn read_temperature(&self, wire: &mut OneWire, delay: &mut DelayUs<u16>) -> Result<f32, Error> {
        let mut scratchpad = [0u8; 9];
        wire.reset_select_write_read(delay, &self.device, &[Command::ReadScratchpad as u8], &mut scratchpad[..])?;
        super::ensure_correct_rcr8(&self.device,&scratchpad[..8], scratchpad[8])?;
        Ok(DS18B20::read_temperature_from_scratchpad(&scratchpad))
    }

    fn read_temperature_from_scratchpad(scratchpad: &[u8]) -> f32 {
        let temp_u16 = LittleEndian::read_u16(&scratchpad[0..2]);
        let temp_f32 = temp_u16 as i16 as f32 / 16_f32;
        temp_f32
    }
}

impl Sensor for DS18B20 {
    fn family_code() -> u8 {
        FAMILY_CODE
    }

    fn start_measurement(&self, wire: &mut OneWire, delay: &mut DelayUs<u16>) -> Result<u16, Error> {
        Ok(self.measure_temperature(wire, delay)?.time_ms())
    }

    fn read_measurement(&self, wire: &mut OneWire, delay: &mut DelayUs<u16>) -> Result<f32, Error> {
        self.read_temperature(wire, delay)
    }
}