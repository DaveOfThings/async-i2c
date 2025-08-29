
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_println::println;
use embedded_hal_async::i2c::I2c;
use fixed::types::extra::U16;
use fixed::FixedU16;
type U16Q16 = FixedU16<U16>;


pub struct Shtc3<T: I2c> {
    bus: T,
    addr: u8,
}

const SLEEP_CMD: [u8; 2] = [0xB0, 0x98];
const _RESET_CMD: [u8; 2] = [0x80, 0x5d]; // Reset command
const READ_ID_CMD: [u8; 2] = [0xEF, 0xC8];  // Read ID Register
const WAKE_CMD: [u8; 2] = [0x35, 0x17];
const MEAS_CMD: [u8; 2] = [0x78, 0x66];  // Read T then RH, no clock stretch, normal mode


impl<T: I2c> Shtc3<T> {
    pub fn new(bus: T, addr: u8) -> Shtc3<T> {
        Shtc3 { bus, addr }
    }

    async fn setup(&mut self) {

        let mut rd_buf = [0x00; 3]; // Read ID Response

        loop {
            // Put SHTC3 in sleep mode
            Timer::after(Duration::from_millis(2)).await;
            match self.bus.write(self.addr, &SLEEP_CMD).await {
                Ok(_) => {
                    println!("Writing sleep command to SHTC3 succeeded.");
                    break;
                }
                Err(e) => {
                    println!("Writing sleep command to SHTC3 failed: {e:?}");
                    Timer::after(Duration::from_millis(500)).await;
                }
            }
        }

        // Wake up to read ID register
        let _ = self.bus.write(self.addr, &WAKE_CMD).await;
        Timer::after(Duration::from_millis(2)).await;
        
        match self.bus.write_read(self.addr, &READ_ID_CMD, &mut rd_buf).await {
            Ok(_) => {
                println!("Read of ID register succeeded.");
            }
            Err(e) => {
                println!("Read of ID register failed: {e:?}");
            }
        }

        if (rd_buf[0] & 0x08 == 0x08) && (rd_buf[1] & 0x3F == 0x07) {
            println!("SHTC3 ID verified.");
        }
        else {
            println!("Unrecognized SHTC3 ID: 0x{:02x} 0x{:02x} 0x{:02x}", rd_buf[0], rd_buf[1], rd_buf[2]);
        }

        let _ = self.bus.write(self.addr, &SLEEP_CMD).await;
        Timer::after(Duration::from_millis(2)).await;

        println!("Configured SHTC3.");
    }

    async fn service(&mut self) {
        
        // Wakeup command
        if !self.bus.write(self.addr, &WAKE_CMD).await.is_ok() {
            println!("Failed to write wake command.");
        }
        Timer::after(Duration::from_millis(2)).await;

        // Measure command
        if !self.bus.write(self.addr, &MEAS_CMD).await.is_ok() {
            println!("Failed to write meas command.");
        }   
        Timer::after(Duration::from_millis(14)).await;  

        // Readout data, 6 bytes
        let mut data = [0; 6];
        if !self.bus.read(self.addr, &mut data).await.is_ok() {
            println!("Failed to read measurement data.");
        }

        if !self.bus.write(self.addr, &SLEEP_CMD).await.is_ok() {
            println!("Failed to write Sleep command after data acquisition.");
        }
        Timer::after(Duration::from_millis(2)).await;

        let temp_raw = U16Q16::from_be_bytes(data[0..2].try_into().unwrap());
        let temp = -45.0 + 175.0 * f32::from(temp_raw);
        let rh_raw = U16Q16::from_be_bytes(data[3..5].try_into().unwrap());
        let rh = 100.0 * f32::from(rh_raw);

        println!("temp: {temp:7.2}C, RH: {rh:7.2}%");

    }

    pub async fn task(&mut self) {
        self.setup().await;

        loop {
            Timer::after(Duration::from_millis(1000)).await;
            self.service().await;
        }
    }
}

