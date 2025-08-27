
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_println::println;
use embedded_hal_async::i2c::I2c;


pub struct Icm42670P<T: I2c> {
    bus: T,
    addr: u8,
}


// Registers of IMU
const PWR_MGMT0 : u8 = 0x1F;
// const GYRO_CONFIG0 : u8 = 0x20;

impl<T: I2c> Icm42670P<T> {
    pub fn new(bus: T, addr: u8) -> Icm42670P<T> {
        Icm42670P { bus, addr }
    }

    async fn setup(&mut self) {
        // Configure IMU and read it
        // Set 6-Axis low noise mode (turn on acccel and gyro) 0x1F PWR_MGMT
        // Configure Accel 0, Accel 1
        // Configure Gyro 0, Gyro 1
        // FIFO Config?
        // Int Source?

        let config_buf = [
            PWR_MGMT0,   // Start writing to PWR_MGMT0 reg
            0x0F,   // PWR_MGMT0: RC Osc off, Gyro and Accel in Low Noise Mode
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

    async fn service(&mut self) {
        let ready_query = [0x39];
        let mut ready: [u8; 1] = [0];


        // Check for new data (read 0x39, use DATA_RDY_INT bit)
        let _ = self.bus.write_read(self.addr, &ready_query, &mut ready).await; 
        if (ready[0] & 1) != 0 {

            let data_query = [0x09];
            let mut data_response = [0; 14];

            let _ = self.bus.write_read(self.addr, &data_query, &mut data_response).await;
            println!("AX = {:02X}{:02X}", data_response[2], data_response[3]);
        }

        // Read Raw Accel, Raw Gyro.
    }

    pub async fn task(&mut self) {
        self.setup().await;

        loop {
            Timer::after(Duration::from_millis(5)).await;
            self.service().await;
        }
    }
}

