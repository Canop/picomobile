use {
    crate::*,
    tokio::sync::mpsc,
};

pub async fn sound_player_task(mut tx: mpsc::Receiver<DetectionEvent>) {
    eprintln!("Sound player task started.");
    let ps = PlaySoundCommand {
        // should be in conf soon
        name: "car-horn".to_string().into(),
        volume: Volume::new(100), // 80% volume
    };
    loop {
        match tx.recv().await {
            Some(_) => {
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
            None => {
                eprintln!("Sound player task is terminating.");
                break;
            }
        }
    }
}
