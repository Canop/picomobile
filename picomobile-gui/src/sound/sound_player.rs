use {
    crate::*,
    tokio::sync::{
        broadcast::{
            self,
            error::RecvError,
        },
    },
};

pub async fn sound_player_task(
    mut tx: broadcast::Receiver<DetectionEvent>,
) {
    eprintln!("Sound player task started.");
    let ps = PlaySoundCommand {
        // should be in conf soon
        name: "car-horn".to_string().into(),
        volume: Volume::new(100), // 80% volume
    };
    loop {
        match tx.recv().await {
            Ok(_) => {
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
