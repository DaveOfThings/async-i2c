#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embassy_executor::{Spawner};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::time::Rate;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::Async;
use esp_hal::{
    gpio::{Level, Output, OutputConfig}, 
    i2c::master::I2c, i2c::master::Config,
};
use emb_esp_exp::icm42670p::Icm42670P;
use esp_println::println;

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

// TODO-DW : Create I2C Bus in a way where two devices can be accessed.

#[embassy_executor::task]
async fn imu_task(mut imu: Icm42670P<I2c<'static, Async>>) -> ! {
    loop {
        imu.task().await;
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

    // Create IMU
    const IMU_ADDR: u8 = 0x68;
    let bus = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(100)))
        .unwrap()
        .with_sda(peripherals.GPIO10)
        .with_scl(peripherals.GPIO8)
        .into_async();

    let imu = Icm42670P::new(bus, IMU_ADDR);
    if !spawner.spawn(imu_task(imu)).is_ok() {
        println!("Spawn of IMU task failed!");
    }

    loop {
        // Blink at 1 Hz
        Timer::after(Duration::from_millis(500)).await;
        led_state = !led_state;
        led.set_level(led_state.into());
    }

}
