use {
    crate::*,
    core::str::FromStr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Servo or Motor driving command
    Driving(DrivingCommand),
    /// Toggle the LED
    ToggleLed,
    /// Disconnect the client, but keep the server running
    Bye,
    /// Disconnect the client and shut down the Pico (back to the bootloader)
    Quit,
}

impl From<DrivingCommand> for Command {
    fn from(driving_command: DrivingCommand) -> Self {
        Command::Driving(driving_command)
    }
}

const DEFAULT_STEERING_LEVEL: u8 = 2;

impl FromStr for Command {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();
        let command = parts.next().ok_or("empty command")?;
        match command {
            "led" | "toggle-led" => Ok(Command::ToggleLed),
            "go" | "forward" | "go-forward" => Ok(DrivingCommand::GoForward.into()),
            "back" | "backward" | "go-backward" => Ok(DrivingCommand::GoBackward.into()),
            "l" | "left" => {
                let level = match parts.next() {
                    Some(level_str) => {
                        let level = level_str
                            .parse::<u8>()
                            .map_err(|_| "level not parsable as u8")?;
                        if level > 7 {
                            return Err("level must be between 1 and 7");
                        }
                        level
                    }
                    None => DEFAULT_STEERING_LEVEL,
                };
                Ok(DrivingCommand::Steer {
                    direction: SteeringDirection::Left,
                    level,
                }
                .into())
            }
            "r" | "right" => {
                let level = match parts.next() {
                    Some(level_str) => {
                        let level = level_str
                            .parse::<u8>()
                            .map_err(|_| "level not parsable as u8")?;
                        if level > 7 {
                            return Err("level must be between 1 and 7");
                        }
                        level
                    }
                    None => DEFAULT_STEERING_LEVEL,
                };
                Ok(DrivingCommand::Steer {
                    direction: SteeringDirection::Right,
                    level,
                }
                .into())
            }
            "c" | "center" => Ok(DrivingCommand::Steer {
                direction: SteeringDirection::Center,
                level: 0,
            }
            .into()),
            "bye" => Ok(Command::Bye),
            "stop" => Ok(DrivingCommand::Stop.into()),
            "q" | "quit" => Ok(Command::Quit),
            _ => Err("unknown command"),
        }
    }
}
