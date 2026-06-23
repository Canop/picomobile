use super::ov2640_registers::*;

pub enum Resolution {
    R160x120,
    R320x240,
    R400x296,
    R640x480,
    R1024x768,
}

impl Resolution {
    pub fn get_sequence(&self) -> &'static [(u8, u8)] {
        match self {
            Resolution::R160x120 => OV2640_160x120_JPEG,
            Resolution::R320x240 => OV2640_320x240_JPEG,
            Resolution::R400x296 => OV2640_400x296_JPEG,
            Resolution::R640x480 => OV2640_640x480_JPEG,
            Resolution::R1024x768 => OV2640_1024x768_JPEG,
        }
    }
}
