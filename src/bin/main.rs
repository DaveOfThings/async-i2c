#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::AnyPin;
use esp_hal::i2c::master::AnyI2c;
use esp_hal::time::Rate;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::{
    gpio::{Level, Output, OutputConfig}, 
    i2c::master::I2c, i2c::master::Config,
};
use esp_println::println;

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
async fn i2c_task(i2c_periph: AnyI2c<'static>, sda: AnyPin<'static>, scl: AnyPin<'static>) -> ! {

    // Create I2C interface
    let mut i2c0 = I2c::new(
        i2c_periph,
        Config::default().with_frequency(Rate::from_khz(100)))
        .unwrap()
        .with_sda(sda)
        .with_scl(scl)
        .into_async();

    let wr_buf = [0x75];    // Who am I request
    let mut rd_buf = [0x00; 1]; // Who Am I response

    let _result = i2c0.write_read_async(0x68, &wr_buf, &mut rd_buf).await;

    println!("Read Who am I: 0x{:02x}", rd_buf[0]);

    // TODO-DW : Configure IMU and read it
    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 0.5.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    // Establish output LED
	let mut led = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
	let mut led_state = false;
    led.set_level(led_state.into());

    // Spawn some tasks
    let _ = spawner.spawn(i2c_task(
        peripherals.I2C0.into(), 
        peripherals.GPIO10.into(), 
        peripherals.GPIO8.into()));

    loop {
        Timer::after(Duration::from_secs(1)).await;
        led_state = !led_state;
        led.set_level(led_state.into());
    }

}
