use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrivingCommand {
    /// Go forward
    GoForward,
    /// Go backward
    GoBackward,
    /// Stop moving
    Stop,
    /// Turn left, right, or center the steering with a certain level of intensity (0-7)
    Steer {
        direction: SteeringDirection,
        level: u8, // in [1-7]
    },
}

pub async fn apply_driving_command(
    command: DrivingCommand,
    motor: &mut Motor<'_>,
    servo: &mut LegoServo<'static>,
) {
    info!("Applying driving command: {:?}", command);
    match command {
        DrivingCommand::GoForward => {
            motor.forward().await;
        }
        DrivingCommand::GoBackward => {
            motor.backward().await;
        }
        DrivingCommand::Stop => {
            motor.stop().await;
        }
        DrivingCommand::Steer { direction, level } => {
            servo.set_position(direction, level).await;
        }
    }
}
