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

/// Command and control interface for the Arducam Mini Plus camera module (OV2640 sensor).
pub struct Arducam<'d> {
    address: u16, // I2C address of the Arducam (OV2640 sensor)
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
        // Default I2C address for OV2640 is 0x30 (7-bit address)
        let address = 0x30;
        Self {
            address,
            i2c,
            spi,
            cs,
        }
    }

    pub async fn init(&mut self) -> Result<(), &'static str> {
        // SPI bus check: write 0x55 to register 0x00 and read it back
        self.write_spi_reg(0x00, 0x55).await;
        if self.read_spi_reg(0x00).await != 0x55 {
            return Err("Arducam SPI bus check failed!");
        }

        // Initialization of sensor OV2640 via I2C

        // Select bank 1 and reset the sensor
        self.write_reg(0xFF, 0x01).await?; // Select bank 1
        self.write_reg(0x12, 0x80).await?; // Reset
        Timer::after_millis(100).await;

        // Init to JPEG
        self.write_regs(OV2640_JPEG_INIT).await?;
        self.write_regs(OV2640_YUV422).await?;
        self.write_regs(OV2640_JPEG).await?;

        // Set the right resolution
        let jpeg_resolution_sequence = OV2640_160x120_JPEG;
        //let jpeg_resolution_sequence = OV2640_320x240_JPEG;
        //let jpeg_resolution_sequence = OV2640_400x296_JPEG;
        //let jpeg_resolution_sequence = OV2640_640x480_JPEG;
        //let jpeg_resolution_sequence = OV2640_1024x768_JPEG;
        self.write_regs(jpeg_resolution_sequence).await?;

        Ok(())
    }

    async fn write_reg(
        &mut self,
        reg: u8,
        val: u8,
    ) -> Result<(), &'static str> {
        let address = self.address;
        self.i2c
            .write_async(address, [reg, val])
            .await
            .map_err(|_| "I2C Error")?;
        Ok(())
    }

    async fn write_regs(
        &mut self,
        regs: &[(u8, u8)],
    ) -> Result<(), &'static str> {
        for &(reg, val) in regs {
            self.write_reg(reg, val).await?;
        }
        Ok(())
    }

    pub async fn trigger_capture(&mut self) -> Result<(), &'static str> {
        self.write_spi_reg(ARDUCHIP_FIFO, FIFO_CLEAR_MASK).await;
        self.write_spi_reg(ARDUCHIP_FIFO, FIFO_START_MASK).await;
        let mut timeout_counter = 0;
        loop {
            let status = self.read_spi_reg(ARDUCHIP_TRIG).await;
            if (status & CAP_DONE_MASK) != 0 {
                break;
            }
            timeout_counter += 1;
            if timeout_counter > 50_000 {
                log::error!("Erreur : L'Arducam ne répond pas (CAP_DONE jamais reçu) !");
                return Err("Arducam hardware hang");
            }
            Timer::after_micros(10).await;
        }
        Ok(())
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
