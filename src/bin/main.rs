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
use emb_esp_exp::shtc3::Shtc3;
use esp_println::println;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use static_cell::StaticCell;

static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2c<Async>>> = StaticCell::new();

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
async fn imu_task(mut imu: Icm42670P<I2cDevice<'static, NoopRawMutex, I2c<'static, Async>>>) -> ! {
    loop {
        imu.task().await;
    }
}

#[embassy_executor::task]
async fn th_task(mut th: Shtc3<I2cDevice<'static, NoopRawMutex, I2c<'static, Async>>>) -> ! {
    loop {
        th.task().await;
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


    // Create and configure I2C Peripheral
    let bus = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(400)))
        .unwrap()
        .with_sda(peripherals.GPIO10)
        .with_scl(peripherals.GPIO8)
        .into_async();

    // create I2C Bus with shared access, protected by mutex
    let i2c_bus = I2C_BUS.init(Mutex::new(bus));

    // Create IMU
    const IMU_ADDR: u8 = 0x68;
    let imu = 
        Icm42670P::new(I2cDevice::new(i2c_bus), IMU_ADDR);

    if !spawner.spawn(imu_task(imu)).is_ok() {
        println!("Spawn of IMU task failed!");
    }

    // create Temp, RH sensor
    const TH_ADDR: u8 = 0x70;
    let th = Shtc3::new(I2cDevice::new(i2c_bus), TH_ADDR);
    if !spawner.spawn(th_task(th)).is_ok() {
        println!("Spawn of Temp/RH sensor failed!");
    }

    loop {
        // Blink at 1 Hz
        Timer::after(Duration::from_millis(500)).await;
        led_state = !led_state;
        led.set_level(led_state.into());
    }

}
