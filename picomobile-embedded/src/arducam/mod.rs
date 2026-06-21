mod arducam_stream;
mod ov2640_registers;

pub use arducam_stream::*;

use {
    embassy_rp::{
        gpio::Output,
        i2c::{
            Async,
            I2c,
        },
        peripherals::{
            I2C0,
            SPI0,
        },
        spi::{
            Async as SpiAsync,
            Spi,
        },
    },
    embassy_time::Timer,
    ov2640_registers::*,
};

// Arducam Mini Plus specific registers and commands
pub const ARDUCHIP_TRIG: u8 = 0x41; // capture trigger register
pub const CAP_DONE_MASK: u8 = 0x08; // Bit 3 indicates capture done
pub const ARDUCHIP_FIFO: u8 = 0x04; // FIFO control register
pub const FIFO_CLEAR_MASK: u8 = 0x01; // Clear FIFO
pub const FIFO_START_MASK: u8 = 0x02; // Start capture
pub const BURST_FIFO_READ: u8 = 0x3C; // Burst read command for FIFO

pub struct Arducam<'d> {
    i2c: I2c<'d, I2C0, Async>,
    spi: Spi<'d, SPI0, SpiAsync>, // real SPI, not the PIO version
    cs: Output<'d>,
}

impl<'d> Arducam<'d> {
    pub fn new(
        i2c: I2c<'d, I2C0, Async>,
        spi: Spi<'d, SPI0, SpiAsync>,
        cs: Output<'d>,
    ) -> Self {
        Self { i2c, spi, cs }
    }

    pub async fn init(&mut self) -> Result<(), &'static str> {
        // 1. SPI bus check: write 0x55 to register 0x00 and read it back
        self.write_spi_reg(0x00, 0x55).await;
        if self.read_spi_reg(0x00).await != 0x55 {
            return Err("Arducam SPI bus check failed!");
        }

        // 2. Initialization of sensor OV2640 via I2C
        // Default I2C address for OV2640 is 0x30 (7-bit address)
        const OV2640_ADDR: u16 = 0x30;

        // Select bank 1 and reset the sensor
        self.i2c
            .write_async(OV2640_ADDR, [0xFF, 0x01])
            .await
            .map_err(|_| "I2C Error")?; // Select bank 1
        self.i2c
            .write_async(OV2640_ADDR, [0x12, 0x80])
            .await
            .map_err(|_| "I2C Error")?; // Reset
        Timer::after_millis(100).await;

        //let jpeg_init_sequence = OV2640_160x120_JPEG;
        let jpeg_init_sequence = OV2640_640x480_JPEG;

        for &(reg, val) in jpeg_init_sequence {
            self.i2c
                .write_async(OV2640_ADDR, [reg, val])
                .await
                .map_err(|_| "I2C Init Error")?;
        }

        Ok(())
    }

    pub async fn trigger_capture(&mut self) {
        self.write_spi_reg(ARDUCHIP_FIFO, FIFO_CLEAR_MASK).await;
        self.write_spi_reg(ARDUCHIP_FIFO, FIFO_START_MASK).await;
        loop {
            let status = self.read_spi_reg(ARDUCHIP_TRIG).await;
            if (status & CAP_DONE_MASK) != 0 {
                break;
            }
            Timer::after_millis(5).await; // Évite de saturer le CPU
        }
    }

    pub async fn get_fifo_length(&mut self) -> u32 {
        let len_low = self.read_spi_reg(0x42).await as u32;
        let len_mid = self.read_spi_reg(0x43).await as u32;
        let len_high = self.read_spi_reg(0x44).await as u32;

        (len_high << 16) | (len_mid << 8) | len_low
    }

    async fn read_spi_reg(
        &mut self,
        reg: u8,
    ) -> u8 {
        let mut buf = [reg & 0x7F, 0x00]; // 1 byte for the register address (bit 7 = 0 for read),
                                          // 1 byte for the value to read
        self.cs.set_low();
        let _ = self.spi.transfer_in_place(&mut buf).await;
        self.cs.set_high();
        buf[1]
    }

    async fn write_spi_reg(
        &mut self,
        reg: u8,
        val: u8,
    ) {
        self.cs.set_low();
        let _ = self.spi.write(&[reg | 0x80, val]).await;
        self.cs.set_high();
    }
}
