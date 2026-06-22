use {
    embassy_rp::gpio::Output,
    embassy_time::{
        Duration,
        Timer,
    },
};

/// A simple motor driver that can be used to control a motor using two GPIO pins.
pub struct Motor<'d> {
    pub p1: Output<'d>,
    pub p2: Output<'d>,
}
impl<'d> Motor<'d> {
    pub fn new(
        p1: Output<'d>,
        p2: Output<'d>,
    ) -> Self {
        Self { p1, p2 }
    }

    // If you have reversed the polarity of the motor, you can swap the forward and backward
    // methods to make it work without changing the wiring.
    pub async fn forward(&mut self) {
        self.p1.set_low();
        self.p2.set_high();
    }

    pub async fn backward(&mut self) {
        self.p1.set_high();
        self.p2.set_low();
    }

    pub async fn stop(&mut self) {
        self.p1.set_low();
        self.p2.set_low();
    }

    pub async fn go_millis(
        &mut self,
        ms: i32,
    ) {
        if ms > 0 {
            self.forward().await;
            Timer::after(Duration::from_millis(ms as u64)).await;
        } else if ms < 0 {
            self.backward().await;
            Timer::after(Duration::from_millis((-ms) as u64)).await;
        }
        self.stop().await;
    }
}
