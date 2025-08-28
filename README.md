# async-i2c
This is an example of how to interface with an i2c peripheral using Rust no-std and async, on an ESP32C3.


## Requirements
The project here runs on the [DevKit-Rust](https://www.espressif.com/en/dev-board/esp32-c3-devkit-rust-1-en) board.

Setup instructions for the board and appropriate tools can be found in the Ferrous Systems Embedded Rust training materials. (Both [std](https://docs.esp-rs.org/std-training/) and [no-std](https://docs.espressif.com/projects/rust/no_std-training/) variants are available.)

## The Challenge
Using the training materials above and the esp-rs/esp-hal documentation, I was able to create a single main.rs file that initialized an i2c peripheral and used it to read and write the IMU on the DevKit-Rust board.  But I wanted to separate the IMU code from main.rs and not use the esp-hal specific interface.  Also, I wanted to share the i2c bus between multiple devices, and the code at this time could only support one.

Trying to search for a better alternative, I encountered a number of Rust crates that I had to figure out.  Some dead end paths were explored and frustration was starting to build.  But I did eventually find the solution I needed.  So here I want to document what I learned as well as the end result.

### embedded-hal
This crate serves as a universal interface between Rust and the embedded hardware.  It's abstract, though, generally defining Traits rather than hardware-specific drivers.  Hardware vendors or others, then, can provide hardware drivers that implement the Traits.  Code that depends only on the Traits is more universal.

Within this crate, embedded-hal::i2c::I2c is the trait defining a blocking API for an I2C peripheral.  The embedded-hal discusses bus sharing, pointing out that the I2c trait could represent either a bus or a single device on a shared bus.  The underlying implementation for this would be out of the scope of the embedded-hal.

What this told me was that my IMU code should be generic and target the I2c trait.  Except that was a blocking API, not async.  But an async analog of the embedded-hal was available.

### embedded-hal-async
Similar to embedded-hal, this provides async functionality in a hardware-neutral way.

So, as mentioned above, the IMU code should be generic and target the I2c trait.  But this would be the embedded_hal_async::i2c::I2c trait instead of the embedded_hal::i2c::I2c trait.

### embedded-hal-bus
This crate appeared to have support for sharing an I2C bus between multiple devices.  I found out, however, that it didn't support async.  So trying to use this was a dead end for me.  

### embassy-embedded-hal
The solution that finally did work, for bus sharing, was in embassy-embedded-hal.  The struct embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice supported bus sharing and implemented the I2c trait needed for the generic IMU.

So with all the components described, we can now present the full picture.

## Code Description

### main.rs
Within src/bin/main.rs, in the function main(), we see the initialization of the I2c peripheral.
```rust
    // Create and configure I2C Peripheral
    let bus = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(100)))
        .unwrap()
        .with_sda(peripherals.GPIO10)
        .with_scl(peripherals.GPIO8)
        .into_async();

    // create I2C Bus with shared access, protected by mutex
    let i2c_bus = I2C_BUS.init(Mutex::new(bus));
```

The I2c::new method here refers to the struct esp_hal::i2c::master::I2c.  (With so many crates defining "I2c", it's hard to keep them separate.)  So this creates a variable representing the I2C peripheral, I2C0, and configures it for 100kHz, sets GPIO10 and GPIO8 as SDA and SCL.  It also sets up the variable for async functionality.

i2c_bus is then created for Mutex-managed access to this peripheral.

Now that we have an i2c bus supporting multiple devices, we can create our IMU:
```rust
    // Create IMU
    const IMU_ADDR: u8 = 0x68;
    let imu = 
        Icm42670P::new(I2cDevice::new(i2c_bus), IMU_ADDR);
```

This uses I2cDevice to create a client of the shared i2c_bus.  And that is passed to our IMU driver, Icm42670P::new().  The compiler infers the type of this imu as Icm42670P<I2cDevice<\'_, NOOPRAWMUTEX, _>> and since embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice implements the embedded_hal_async::i2c::I2c trait, everything is happy.

Finally, main spawns an async task to run the IMU interface:
```rust
    if !spawner.spawn(imu_task(imu)).is_ok() {
        println!("Spawn of IMU task failed!");
    }
```

### icm42670p.rs

Unlike main.rs, the code here is relatively straightforward.  We define the IMU struct as generic with the I2c trait.  (That's embedded_hal_async::i2c::I2c)

```Rust
pub struct Icm42670P<T: I2c> {
    bus: T,
    addr: u8,
}
```

Then it's a simple matter of writing async methods to set up the IMU and to poll it.  There's also an async method to do the setup and repeatedly poll in a loop, comprising the entire async task needed to run the IMU.

```Rust

    async fn setup(&mut self) {

        let config_buf = [
            PWR_MGMT0,   // Start writing to PWR_MGMT0 reg
            0x0F,   // PWR_MGMT0: RC Osc off, Gyro and Accel on in Low Noise Mode
            0x69,   // GYRO_CONFIG0: +/-250 dps, Gyro ODR: 100Hz 
            0x69,   // ACCEL_CONFIG0: +/- 2g, Accel ODR: 100Hz
            0x60,   // TEMP_CONFIG0: LPF: 4 Hz
            0x05,   // GYRO_CONFIG1: LPF: 34 Hz
            0x05,   // ACCEL_CONFIG1: 2x averaging for LPM, 34 Hz LPF
            ];
        let _ = self.bus.write(self.addr, &config_buf).await;
        println!("Configured IMU.");

        let wr_buf = [0x75];    // Who am I request
        let mut rd_buf = [0x00; 1]; // Who Am I response

        let _result = self.bus.write_read(self.addr, &wr_buf, &mut rd_buf).await;

        println!("Who am I: 0x{:02x}", rd_buf[0]);
    }
```

```Rust

    async fn service(&mut self) {
        let ready_query = [0x39];
        let mut ready: [u8; 1] = [0];


        // Check for new data (read 0x39, use DATA_RDY_INT bit)
        let _ = self.bus.write_read(self.addr, &ready_query, &mut ready).await; 
        if (ready[0] & 1) != 0 {

            // Read Raw Accel, Raw Gyro.
            let data_query = [0x09];
            let mut data_response = [0; 14];

            let _ = self.bus.write_read(self.addr, &data_query, &mut data_response).await;
            let ax: F16U14 = F16U14::from_be_bytes(data_response[2..4].try_into().unwrap());
            let ay: F16U14 = F16U14::from_be_bytes(data_response[4..6].try_into().unwrap());
            let az: F16U14 = F16U14::from_be_bytes(data_response[6..8].try_into().unwrap());

            // Print results
            println!("Acc = {ax:7.4}, {ay:7.4}, {az:7.4}");
        }
    }
```

```Rust

    pub async fn task(&mut self) {
        self.setup().await;

        loop {
            Timer::after(Duration::from_millis(5)).await;
            self.service().await;
        }
    }
```

## Conclusion and Next Steps

It is, indeed, possible to write embedded Rust code to efficiently access and share the i2c bus using async, no-std code.  The process of finding or developing this solution isn't completely obvious, as numerous packages define 'I2c' in different ways and searches turn up crates with async and blocking APIs.  

But here are all the pieces in one place and working.

This code should be portable to other processors than the ESP32.  In main.rs, the call to I2c::new would need to change to whatever the alternative platform provides.  The rest depends only on embassy-embedded-hal and embedded-hal-async.

While this code is structured to support multiple i2c devices, it still only accesses one.  I hope to add the temperature and humidity sensors soon.

