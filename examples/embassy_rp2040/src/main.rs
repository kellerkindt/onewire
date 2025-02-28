#![no_std]
#![no_main]

use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, OutputOpenDrain};
use embassy_time::{Delay, Duration, Timer};
use onewire::{DS18B20, DeviceSearch, OneWire};

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Prepare pin for use with the library
    let mut ow_pin = OutputOpenDrain::<'static>::new(p.PIN_14, Level::Low);
    let mut wire = OneWire::new(&mut ow_pin, false);

    let mut first_iteration = true;
    'infinite: loop {
        // Just a little trick to prevent spam, in case we need to restart this loop
        if !first_iteration {
            Timer::after(Duration::from_secs(1)).await; // Prevent spam
        } else {
            first_iteration = false;
        }

        // Reset to test if wire is okay and if any sensor is connected
        if wire
            .reset(&mut Delay)
            .inspect_err(|err| error!("Failed to reset wire: {}", err))
            .is_err()
        {
            continue 'infinite;
        }

        // Start searching for a sensor (we just care to get the first one)
        let mut search = DeviceSearch::new();
        let Ok(device) = wire
            .search_next(&mut search, &mut Delay)
            .inspect_err(|err| error!("Failed to search for temperature sensor: {}", err))
        else {
            continue;
        };
        let Some(device) = device else {
            info!("No temperature sensor found");
            continue;
        };

        info!("Found temperature sensor: {:?}", device);

        // Construct the sensor driver
        let Ok(sensor) = DS18B20::new(device)
            .inspect_err(|err| error!("Failed to create temperature sensor: {}", err))
        else {
            continue;
        };

        'measure: loop {
            // Start a measurement
            let Ok(resolution) = sensor
                .measure_temperature(&mut wire, &mut Delay)
                .inspect_err(|err| error!("Failed to start temperature measurement: {}", err))
            else {
                continue 'measure;
            };

            // After starting a measurement, we need to wait for the measurement to finish
            // The time we need to wait, depends on the resolution (more resolution takes longer)
            Timer::after(Duration::from_millis(resolution.time_ms() as u64)).await;

            // Retrieve the measured temperature from the sensor
            let Ok(raw_temperature) = sensor
                .read_temperature(&mut wire, &mut Delay)
                .inspect_err(|err| error!("Failed to read temperature: {}", err))
            else {
                continue 'measure;
            };

            // Process and log the temperature
            let (integer, fraction) = onewire::ds18b20::split_temp(raw_temperature);
            let temperature = (integer as f32) + (fraction as f32) / 10000.0;
            info!("Current temperature: {}", temperature);
        }
    }
}
