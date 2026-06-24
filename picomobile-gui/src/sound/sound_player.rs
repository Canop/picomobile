use {
    crate::*,
    tokio::sync::broadcast::{
        self,
        error::RecvError,
    },
};

pub async fn sound_player_task(
    mut event_rx: broadcast::Receiver<DetectionEvent>,
    config_rx: watch::Receiver<CamConfig>,
) {
    eprintln!("Sound player task started.");
    let ps = PlaySoundCommand {
        // should be in conf soon
        name: "car-horn".to_string().into(),
        volume: Volume::new(100), // 80% volume
    };
    loop {
        match event_rx.recv().await {
            Ok(_) => {
                let config = *config_rx.borrow();
                if !config.play_sound_on_motion {
                    eprintln!("Motion event detected, but sound playback is disabled. Skipping.");
                    continue;
                }
                match play_sound(&ps).await {
                    Ok(()) => {}
                    Err(SoundError::Interrupted) => {
                        // not yet implemented, but just in case
                        eprintln!("sound interrupted");
                        break;
                    }
                    Err(e) => {
                        eprintln!("sound error: {}", e);
                    }
                }
            }
            Err(RecvError::Lagged(count)) => {
                eprintln!("Sound Player lagged by {} messages.", count);
            }
            Err(RecvError::Closed) => {
                eprintln!("Sound Player task is terminating.");
                break;
            }
        }
    }
}
