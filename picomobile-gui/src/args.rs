use {
    clap::Parser,
    std::path::PathBuf,
};

#[derive(Parser)]
pub struct Args {
    /// Car IP address
    #[arg(long)]
    pub car_ip: String,

    /// Web GUI port
    #[arg(long)]
    pub gui_port: Option<u16>,

    /// Command to shot a few images - path to save them
    #[arg(long)]
    pub capture_images: Option<PathBuf>,
}
