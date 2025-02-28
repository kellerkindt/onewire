# embassy rp2040 example
This example has been tested to work correctly on a [Raspberry Pi Pico W](https://www.raspberrypi.com/products/raspberry-pi-pico/) with [this temperature probe](https://www.az-delivery.de/en/products/2xds18b20wasserdicht). 

The Signal wire of the Temperature probe should be connected to PIN/GPIO 14.
Refer to the [pinout](https://www.raspberrypi.com/documentation/microcontrollers/pico-series.html#pinout-and-design-files-4). 

## How to run

1. Install [probe-rs](https://probe.rs/)
2. Connect your [Raspberry Pi Debug Probe](https://www.raspberrypi.com/documentation/microcontrollers/debug-probe.html) to the [Pi Pico W](https://www.raspberrypi.com/products/raspberry-pi-pico/)
3. Connect the Debug Probe to your computer
4. Supply power to the Raspberry Pi Pico (e.g. through the USB port)
5. `cargo run`

The output should look like this:
```
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.10s
     Running `probe-rs run --chip RP2040 target/thumbv6m-none-eabi/debug/embassy_rp2040`
      Erasing ✔ 100% [####################]  12.00 KiB @  48.23 KiB/s (took 0s)
  Programming ✔ 100% [####################]  12.00 KiB @  36.70 KiB/s (took 0s)                                                                                                                                                            Finished in 0.58s
INFO  Found temperature sensor: Device { address: [0x28, 0xff, 0x64, 0x1e, 0x9d, 0x93, 0xae, 0x75] }
└─ embassy_rp2040::____embassy_main_task::{async_fn#0}::{closure#0} @ src/main.rs:38  
INFO  Current temperature: 21.25
└─ embassy_rp2040::____embassy_main_task::{async_fn#0}::{closure#3} @ src/main.rs:64  
...
```