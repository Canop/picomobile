use {
    crate::*,
    std::{
        sync::Arc,
        time::Duration,
    },
    tokio::{
        io::BufReader,
        io::AsyncWriteExt,
        sync::broadcast,
        time::sleep,
    },
};

/// Fetches images from the car's camera and broadcasts them to all connected clients.
pub async fn camera_fetcher_task(
    car_addr: String,
    config_rx: watch::Receiver<CamConfig>,
    tx: broadcast::Sender<Arc<Vec<u8>>>,
    minimal_receivers_for_connection: usize,
) {
    let mut opt_stream = None; // TCP stream to the car's camera

    loop {
        // If there's no client, we don't need to connect to the Pico
        if tx.receiver_count() < minimal_receivers_for_connection {
            if opt_stream.is_some() {
                eprintln!("Not enough subscribers. Closing connection to Pico.");
                opt_stream = None;
            }
            sleep(Duration::from_millis(500)).await;
            continue;
        }

        // When there are clients, we need to ensure we have a connection to the Pico
        if opt_stream.is_none() {
            match tokio::net::TcpStream::connect(&car_addr).await {
                Ok(mut s) => {
                    eprintln!("Connected to car camera stream at {car_addr}");
                    let config = *config_rx.borrow();
                    let resolution = config.resolution.to_string();
                    if let Err(e) = s.write_all(resolution.as_bytes()).await {
                        eprintln!("Failed to send resolution request to car camera: {e}");
                        continue;
                    }
                    opt_stream = Some(BufReader::new(s));
                }
                Err(e) => {
                    eprintln!("Failed to connect to car camera at {car_addr}: {e}");
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
        }

        // Read and broadcast images
        if let Some(ref mut stream) = opt_stream {
            match tokio::time::timeout(Duration::from_secs(2), read_jpeg_from_stream(stream)).await
            {
                Ok(Ok(frame)) => {
                    if let Err(e) = tx.send(Arc::new(frame)) {
                        eprintln!("Failed to broadcast frame: {e}");
                    }
                }
                Ok(Err(e)) => {
                    eprintln!(
                        "Pico camera connection lost (read error): {e}. Attempting to reconnect..."
                    );
                    opt_stream = None;
                }
                Err(_timeout_error) => {
                    eprintln!(
                        "Pico camera connection timed out (no data received). Reconnecting..."
                    );
                    opt_stream = None; // triggers reconnection
                }
            }
        }
    }
}
