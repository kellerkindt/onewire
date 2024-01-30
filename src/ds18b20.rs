use byteorder::ByteOrder;
use byteorder::LittleEndian;
use core::fmt::Debug;
use hal::delay::DelayNs;

use crate::Error;
use crate::OneWire;
use crate::Sensor;
use crate::{Device, OpenDrainOutput};
use core::convert::Infallible;

pub const FAMILY_CODE: u8 = 0x28;

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
    TC = 0b0111_1111,
}

impl MeasureResolution {
    pub fn time_ms(&self) -> u16 {
        match self {
            MeasureResolution::TC8 => 94,
            MeasureResolution::TC4 => 188,
            MeasureResolution::TC2 => 375,
            MeasureResolution::TC => 750,
        }
    }
}

pub struct DS18B20 {
    device: Device,
    resolution: MeasureResolution,
}

impl DS18B20 {
    pub fn new(device: Device) -> Result<DS18B20, Error<Infallible>> {
        if device.address[0] != FAMILY_CODE {
            Err(Error::FamilyCodeMismatch(FAMILY_CODE, device.address[0]))
        } else {
            Ok(DS18B20 {
                device,
                resolution: MeasureResolution::TC,
            })
        }
    }

    /// # Safety
    ///
    /// This is marked as unsafe because it does not check whether the given address
    /// is compatible with a DS18B20 device. It assumes so.
    pub unsafe fn new_forced(device: Device) -> DS18B20 {
        DS18B20 {
            device,
            resolution: MeasureResolution::TC,
        }
    }

    pub fn measure_temperature<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<MeasureResolution, Error<O::Error>> {
        wire.reset_select_write_only(delay, &self.device, &[Command::Convert as u8])?;
        Ok(self.resolution)
    }

    pub fn read_temperature<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<u16, Error<O::Error>> {
        let mut scratchpad = [0u8; 9];
        wire.reset_select_write_read(
            delay,
            &self.device,
            &[Command::ReadScratchpad as u8],
            &mut scratchpad[..],
        )?;
        super::ensure_correct_rcr8(&self.device, &scratchpad[..8], scratchpad[8])?;
        Ok(DS18B20::read_temperature_from_scratchpad(&scratchpad))
    }

    fn read_temperature_from_scratchpad(scratchpad: &[u8]) -> u16 {
        LittleEndian::read_u16(&scratchpad[0..2])
    }
}

impl Sensor for DS18B20 {
    fn family_code() -> u8 {
        FAMILY_CODE
    }

    fn start_measurement<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<u16, Error<O::Error>> {
        Ok(self.measure_temperature(wire, delay)?.time_ms())
    }

    fn read_measurement<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<f32, Error<O::Error>> {
        self.read_temperature(wire, delay)
            .map(|t| t as i16 as f32 / 16_f32)
    }

    fn read_measurement_raw<O: OpenDrainOutput>(
        &self,
        wire: &mut OneWire<O>,
        delay: &mut impl DelayNs,
    ) -> Result<u16, Error<O::Error>> {
        self.read_temperature(wire, delay)
    }
}

/// Split raw u16 value to two parts: integer and fraction N
/// Original value may be calculated as: integer + fraction/10000
pub fn split_temp(temperature: u16) -> (i16, i16) {
    if temperature < 0x8000 {
        (temperature as i16 >> 4, (temperature as i16 & 0xF) * 625)
    } else {
        let abs = -(temperature as i16);
        (-(abs >> 4), -625 * (abs & 0xF))
    }
}

#[cfg(test)]
mod tests {
    use super::split_temp;
    #[test]
    fn test_temp_conv() {
        assert_eq!(split_temp(0x07d0), (125, 0));
        assert_eq!(split_temp(0x0550), (85, 0));
        assert_eq!(split_temp(0x0191), (25, 625)); // 25.0625
        assert_eq!(split_temp(0x00A2), (10, 1250)); // 10.125
        assert_eq!(split_temp(0x0008), (0, 5000)); // 0.5
        assert_eq!(split_temp(0x0000), (0, 0)); // 0
        assert_eq!(split_temp(0xfff8), (0, -5000)); // -0.5
        assert_eq!(split_temp(0xFF5E), (-10, -1250)); // -10.125
        assert_eq!(split_temp(0xFE6F), (-25, -625)); // -25.0625
        assert_eq!(split_temp(0xFC90), (-55, 0)); // -55
    }
}
