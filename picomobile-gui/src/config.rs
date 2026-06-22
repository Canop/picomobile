use serde::{
    Deserialize,
    Serialize,
};

/// State of motion detection configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct MotionDetectionConfig {
    pub enable_motion_detection: bool,
    pub sound_on_motion: bool,
    pub save_motion_events: bool,
}

/// A request to update the motion detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMotionDetectionConfig {
    pub enable_motion_detection: Option<bool>,
    pub sound_on_motion: Option<bool>,
    pub save_motion_events: Option<bool>,
}
