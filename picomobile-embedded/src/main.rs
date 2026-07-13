#![no_std]
#![no_main]

mod arducam;
mod command;
mod driving;
mod motor;
mod servo;
mod slice_writer;
mod utils;

pub use {
    arducam::*,
    command::*,
    driving::*,
    motor::*,
    servo::*,
    slice_writer::*,
    utils::*,
};

use {
    core::{
        fmt::Write as _,
        str::from_utf8,
    },
    cyw43::{
        JoinOptions,
        aligned_bytes,
    },
    cyw43_pio::{
        DEFAULT_CLOCK_DIVIDER,
        PioSpi,
    },
    embassy_executor::Spawner,
    embassy_net::{
        Config,
        StackResources,
        tcp::TcpSocket,
    },
    embassy_rp::{
        bind_interrupts,
        clocks::RoscRng,
        dma,
        gpio::{
            Level,
            Output,
        },
        i2c,
        peripherals::{
            DMA_CH0,
            DMA_CH1,
            DMA_CH2,
            DMA_CH3,
            DMA_CH4,
            I2C0,
            PIO0,
            USB,
        },
        pio::{
            InterruptHandler,
            Pio,
        },
        pwm::Pwm,
        usb::{
            Driver as UsbDriver,
            InterruptHandler as UsbIrqHandler,
        },
    },
    embassy_sync::{
        blocking_mutex::raw::CriticalSectionRawMutex,
        channel::Channel,
    },
    embassy_time::{
        Duration,
        Timer,
        with_timeout,
    },
    embedded_io_async::Write,
    log::{
        info,
        warn,
    },
    panic_halt as _,
    rp2040_hal::rom_data::reset_to_usb_boot,
    static_cell::StaticCell,
};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>,
                 dma::InterruptHandler<DMA_CH1>,
                 dma::InterruptHandler<DMA_CH2>,
                 dma::InterruptHandler<DMA_CH3>,
                 dma::InterruptHandler<DMA_CH4>;
    USBCTRL_IRQ => UsbIrqHandler<USB>;
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
});

static COMMAND_CHANNEL: Channel<CriticalSectionRawMutex, DrivingCommand, 2> = Channel::new();

const WIFI_NETWORK: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

/// Execute driving commands coming on a channel, with a timeout to stop the
/// motor if no command is received for a while.
#[embassy_executor::task]
async fn driving_task(
    mut motor: Motor<'static>,
    mut servo: LegoServo<'static>,
) {
    let timeout_duration = Duration::from_millis(200);
    loop {
        let cmd = with_timeout(timeout_duration, COMMAND_CHANNEL.receive()).await;
        match cmd {
            Ok(command) => {
                info!("DRIVING COMMAND: {:?}", command);
                apply_driving_command(command, &mut motor, &mut servo).await;
            }
            Err(_timeout) => {
                info!("driving TIMEOUT");
                // Timeout or other command, stop the motor
                motor.stop().await;
                // In order not to loop for nothing and consuming CPU,
                // we block waiting for the next command
                let command = COMMAND_CHANNEL.receive().await;
                info!("FIRST DRIVING COMMAND: {:?}", command);
                apply_driving_command(command, &mut motor, &mut servo).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<
        'static,
        cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>,
        cyw43::Cyw43439,
    >
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn logger_task(driver: UsbDriver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn tick() {
    let mut i = 0;
    loop {
        Timer::after_millis(60_000).await;
        i += 1;
        log::info!("tick {i}");
    }
}
async fn blink(
    led: &mut Output<'static>,
    n: usize,
) {
    let before = led.is_set_high();
    for _ in 0..n {
        led.set_high();
        Timer::after_millis(150).await;
        led.set_low();
        Timer::after_millis(150).await;
    }
    if before {
        led.set_high();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Logger USB — must be before everything else
    let usb_driver = UsbDriver::new(p.USB, Irqs);
    spawner.spawn(expect(logger_task(usb_driver)).await);

    // LED on pin 27 of the Kitronik 5331, active=high
    let mut led = Output::new(p.PIN_27, Level::Low);

    blink(&mut led, 1).await;
    Timer::after_millis(1000).await;

    blink(&mut led, 1).await;
    info!("Starting...");

    // Motor on pins Motor1 of the Kitronik 5331, which are mapped
    // to pins 2 and 3 of the RP2040
    let motor = Motor::new(
        Output::new(p.PIN_2, Level::Low),
        Output::new(p.PIN_3, Level::Low),
    );

    // Servo on pins Motor2 of the Kitronik 5331,
    // which are mapped to pins 6 and 7 of the RP2040,
    // pins which are handled by the PWM slice 3
    let pwm = Pwm::new_output_ab(p.PWM_SLICE3, p.PIN_6, p.PIN_7, LegoServo::pwm_config());
    let (channel_a, channel_b) = pwm.split();
    let servo = LegoServo {
        channel_a: channel_a.unwrap(), // safe ? probably...
        channel_b: channel_b.unwrap(),
    };

    // Configuration of the Arducam (pins from GP16 to GP21)
    let mut i2c_config = embassy_rp::i2c::Config::default();
    i2c_config.frequency = 100_000;
    let i2c0 = embassy_rp::i2c::I2c::new_async(p.I2C0, p.PIN_21, p.PIN_20, Irqs, i2c_config);

    let mut spi_config = embassy_rp::spi::Config::default();
    spi_config.frequency = 4_000_000; // 8 MHz is possible but may lead to parasites
    let spi0 = embassy_rp::spi::Spi::new(
        p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, p.DMA_CH3, p.DMA_CH4, Irqs, spi_config,
    );
    let cs_pin = Output::new(p.PIN_17, Level::High);
    let arducam = Arducam::new(i2c0, spi0, cs_pin);

    spawner.spawn(expect(driving_task(motor, servo)).await);
    spawner.spawn(expect(tick()).await);

    let mut rng = RoscRng;

    let fw = aligned_bytes!("../../cyw43-firmware/43439A0.bin");
    let clm = aligned_bytes!("../../cyw43-firmware/43439A0_clm.bin");
    let nvram = aligned_bytes!("../../cyw43-firmware/nvram_rp2040.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let pio_spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        dma::Channel::new(p.DMA_CH0, Irqs),
        dma::Channel::new(p.DMA_CH1, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, pio_spi, fw, nvram).await;
    spawner.spawn(expect(cyw43_task(runner)).await);

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::Performance)
        .await;

    let config = Config::dhcpv4(Default::default());

    // Generate random seed
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    Timer::after_millis(150).await;
    spawner.spawn(expect(net_task(runner)).await);

    Timer::after_millis(150).await;
    spawner.spawn(expect(camera_streaming_task(stack, arducam)).await);

    while let Err(err) = control
        .join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
        .await
    {
        info!("join failed: {:?}", err);
    }

    info!("waiting for link...");
    stack.wait_link_up().await;

    info!("waiting for DHCP...");
    stack.wait_config_up().await;

    info!("Network stack is up!");

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut buf = [0; 4096];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        control.gpio_set(0, false).await;
        info!("Listening on TCP:1234...");
        blink(&mut led, 1).await;
        if let Err(e) = socket.accept(1234).await {
            warn!("accept error: {:?}", e);
            continue;
        }

        info!(
            "Received driving connection from {:?}",
            socket.remote_endpoint()
        );
        control.gpio_set(0, true).await;

        loop {
            let _ = socket.write_all(b"\n> ").await;
            let n = match socket.read(&mut buf).await {
                Ok(0) => {
                    warn!("read EOF");
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    warn!("read error: {:?}", e);
                    break;
                }
            };
            let Ok(input) = from_utf8(&buf[..n]) else {
                warn!("Received non-UTF8 data");
                continue;
            };

            info!(" <- Received '{}'", input);
            let cmd = input.parse::<Command>();

            let mut output = SliceWriter::new(&mut buf);
            let mut close = false;
            let mut quit = false;
            match cmd {
                Ok(Command::ToggleLed) => {
                    if led.is_set_high() {
                        led.set_low();
                        let _ = write!(output, "-> LED is now OFF");
                    } else {
                        led.set_high();
                        let _ = write!(output, "-> LED is now ON");
                    }
                }
                Ok(Command::Driving(driving_command)) => {
                    COMMAND_CHANNEL.send(driving_command).await;
                }
                Ok(Command::Bye) => {
                    let _ = write!(output, "-> Goodbye!");
                    close = true;
                }
                Ok(Command::Quit) => {
                    let _ = write!(output, "-> Shutting down...");
                    quit = true;
                }
                Err(e) => {
                    let _ = write!(output, "-> ERROR: {}", e);
                }
            }
            match socket.write_all(output.as_bytes()).await {
                Ok(()) => {
                    info!("Answered '{}'", output.as_str());
                }
                Err(e) => {
                    warn!("write error: {:?}", e);
                    if !quit {
                        break;
                    }
                }
            };
            if close {
                info!("Closing connection");
                break;
            }
            if quit {
                // we blink the LED a few times to give the user a visual feedback that the
                // command was received, and so that the log can reach tio before the reset
                // happens
                blink(&mut led, 5).await;
                reset_to_usb_boot(0, 0);
            }
        }
        Timer::after_millis(10).await;
    }
}
