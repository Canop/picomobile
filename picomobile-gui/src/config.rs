use {
    crate::*,
    serde::{
        Deserialize,
        Serialize,
    },
};

/// State of motion detection configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct CamConfig {
    pub resolution: Resolution,
    pub enable_motion_detection: bool,
    pub play_sound_on_motion: bool,
    pub save_motion_events: bool,
}

/// A request to update the motion detection configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct UpdateCamConfig {
    #[serde(default)]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub enable_motion_detection: Option<bool>,
    #[serde(default)]
    pub play_sound_on_motion: Option<bool>,
    #[serde(default)]
    pub save_motion_events: Option<bool>,
}

impl CamConfig {
    /// Update the configuration with the provided values.
    pub fn update(
        &mut self,
        update: UpdateCamConfig,
    ) {
        if let Some(resolution) = update.resolution {
            self.resolution = resolution;
        }
        if let Some(enable_motion_detection) = update.enable_motion_detection {
            self.enable_motion_detection = enable_motion_detection;
        }
        if let Some(play_sound_on_motion) = update.play_sound_on_motion {
            self.play_sound_on_motion = play_sound_on_motion;
        }
        if let Some(save_motion_events) = update.save_motion_events {
            self.save_motion_events = save_motion_events;
        }
    }
}
