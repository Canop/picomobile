use {
    crate::*,
    std::path::PathBuf,
    tokio::sync::broadcast::{
        self,
        error::RecvError,
    },
};

pub async fn event_saver_task(
    mut tx: broadcast::Receiver<DetectionEvent>,
) {
    let events_root = PathBuf::from("events");
    if !events_root.exists() {
        if let Err(e) = tokio::fs::create_dir_all(&events_root).await {
            eprintln!("Failed to create events directory: {}", e);
            return;
        }
    }
    loop {
        match tx.recv().await {
            Ok(DetectionEvent { time, images }) => {
                let time_str = time.to_string();
                let event_dir = events_root.join(time_str);
                if let Err(e) = tokio::fs::create_dir_all(&event_dir).await {
                    eprintln!("Failed to create event directory: {}", e);
                    continue;
                }
                for (i, image) in images.iter().enumerate() {
                    let image_path = event_dir.join(format!("frame_{}.jpg", i));
                    if let Err(e) = tokio::fs::write(&image_path, &**image).await {
                        eprintln!("Failed to save image {}: {}", image_path.display(), e);
                    }
                }
            }
            Err(RecvError::Lagged(count)) => {
                eprintln!("Event Saver lagged by {} messages.", count);
            }
            Err(RecvError::Closed) => {
                eprintln!("Event Saver task is terminating.");
                break;
            }
        }
    }
}
