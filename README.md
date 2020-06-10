# OneWire

This crate is an OneWire-Bus implementation ontop of generic `Input-` and `OutputPins` from the [embedded-hal](https://crates.io/crates/embedded-hal).

[![Build Status](https://github.com/kellerkindt/onewire/workflows/Rust/badge.svg)](https://github.com/kellerkindt/onewire/actions?query=workflow%3ARust)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/kellerkindt/onewire)
[![Crates.io](https://img.shields.io/crates/v/onewire.svg)](https://crates.io/crates/onewire)
[![Documentation](https://docs.rs/onewire/badge.svg)](https://docs.rs/onewire)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](https://github.com/kellerkindt/onewire/issues/new)



# How to use
Below is an example how to create a new OneWire instance, search for devices and read the temperature from a [DS18B20](https://www.google.com/url?sa=t&rct=j&q=&esrc=s&source=web&cd=1&cad=rja&uact=8&ved=0ahUKEwjY3ZaK3ZTcAhUwb5oKHeW1AaYQFghhMAA&url=https%3A%2F%2Fdatasheets.maximintegrated.com%2Fen%2Fds%2FDS18B20.pdf&usg=AOvVaw1BHiiWuK-ej9DummvLpx8c).
The example currently requires the stm32f103xx-hal to be patched with this [PR](https://github.com/japaric/stm32f103xx-hal/pull/51).

```rust
fn main() -> ! {
    let mut cp: cortex_m::Peripherals = cortex_m::Peripherals::take().unwrap();
    let mut peripherals = stm32f103xx::Peripherals::take().unwrap();
    let mut flash = peripherals.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut rcc = peripherals.RCC.constrain();
    let mut gpioc = peripherals.GPIOC.split(&mut rcc.apb2);
    
    let mut delay = stm32f103xx_hal::delay::Delay::new(cp.SYST, clocks);
    
    let mut one = gpioc
        .pc15
        .into_open_drain_output(&mut gpioc.crh)
        .downgrade();
        
    let mut wire = OneWire::new(&mut one, false);
    
    if wire.reset(&mut delay).is_err() {
        // missing pullup or error on line
        loop {}
    }
    
    // search for devices
    let mut search = DeviceSearch::new();
    while let Some(device) = wire.search_next(&mut search, &mut delay).unwrap() {
        match device.address[0] {
            ds18b20::FAMILY_CODE => {
                let mut ds18b20 = DS18b20::new(device).unwrap();
                
                // request sensor to measure temperature
                let resolution = ds18b20.measure_temperature(&mut wire, &mut delay).unwrap();
                
                // wait for compeltion, depends on resolution 
                delay.delay_ms(resolution.time_ms());
                
                // read temperature
                let temperature = ds18b20.read_temperature(&mut wire, &mut delay).unwrap();
            },
            _ => {
                // unknown device type            
            }
        }
    }
    
    loop {}
}
```
The code from the example is copy&pasted from a working project, but not tested in this specific combination. 
