use {
    crate::expect,
    embassy_rp::pwm::{
        Config,
        PwmOutput,
        SetDutyCycle,
    },
    fixed::types::U12F4,
    log::info,
};

/// Represents the steering direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteeringDirection {
    Center,
    Left,
    Right,
}

/// Allow controlling a servo using the Motor2 pins with PWM.
pub struct LegoServo<'d> {
    pub channel_a: PwmOutput<'d>,
    pub channel_b: PwmOutput<'d>,
}
impl<'d> LegoServo<'d> {
    /// Build a configuration for the PWM slice 7 that allows to control the servo.
    ///
    /// The frequency of the servo is 1200 Hz while the clock of the Pico (sys_clk) is at 125 MHz.
    /// The formula is
    ///     frequency = sys_clk / (divider * top)
    ///  -> 125_000_000 / (4 * 26041) = 1200 Hz/
    ///     and divider * top must fit in 16 bits
    /// (I didn't come with this divider of 4, it seems to be the usual practice)
    pub fn pwm_config() -> Config {
        let mut pwm_config = Config::default();
        pwm_config.divider = U12F4::from_num(4);
        pwm_config.top = 26041;
        pwm_config
    }

    /// Brings the servo back to its center position (0% duty cycle on both channels).
    pub fn center(&mut self) {
        let _ = self.channel_a.set_duty_cycle_fully_off();
        let _ = self.channel_b.set_duty_cycle_fully_off();
        info!("Set steering to CENTER with duty cycle 0% on both channels");
    }

    /// Sets the servo position.
    ///
    /// * `dir`: The direction to turn (`Left` or `Right`).
    /// * `level`: The intensity of the turn from `0` (center) to `7` (maximum lock).
    pub async fn set_position(
        &mut self,
        dir: SteeringDirection,
        level: u8,
    ) {
        // Clamp the input level to the maximum supported Lego steps
        let level = level.min(7);

        if level == 0 {
            self.center();
            return;
        }

        // Levels which seem to correspond to the Lego steering steps
        let duty_num = match level {
            1 => 266,  // 26.6%
            2 => 389,  // 38.9%
            3 => 512,  // 51.2%
            4 => 630,  // 63.0%
            5 => 750,  // 75.0%
            6 => 872,  // 87.2%
            _ => 1000, // 100.0%
        };

        // Apply PWM to one channel while forcing the other to ground (0)
        // Note: If Left/Right logic is inverted relative to your chassis build,
        // simply swap the channel_a and channel_b assignments below.
        match dir {
            SteeringDirection::Center => self.center(),
            SteeringDirection::Left => {
                expect(self.channel_a.set_duty_cycle_fully_off()).await;
                expect(self.channel_b.set_duty_cycle_fraction(duty_num, 1000)).await;
            }
            SteeringDirection::Right => {
                expect(self.channel_b.set_duty_cycle_fully_off()).await;
                expect(self.channel_a.set_duty_cycle_fraction(duty_num, 1000)).await;
            }
        }
    }
}
