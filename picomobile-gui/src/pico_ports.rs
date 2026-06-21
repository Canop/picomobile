/// TCP ports used for communication with the Pico
#[derive(Clone, Copy, Debug)]
pub struct PicoPorts {
    /// Port for sending commands to the Pico
    pub command_port: u16,
    /// Port for receiving images from the Pico
    pub image_port: u16,
}
