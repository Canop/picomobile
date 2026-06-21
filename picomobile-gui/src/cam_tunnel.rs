use {
    crate::*,
    std::{
        sync::Arc,
        time::Duration,
    },
    tokio::{
        io::BufReader,
        time::sleep,
        sync::broadcast,
    },
};

/// Fetches images from the car's camera and broadcasts them to all connected clients.
pub async fn camera_fetcher_task(
    car_addr: String,
    tx: broadcast::Sender<Arc<Vec<u8>>>,
) {
    let mut opt_stream = None; // TCP stream to the car's camera

    loop {
        // 1. If there's no client, we don't need to connect to the Pico
        if tx.receiver_count() == 0 {
            if opt_stream.is_some() {
                eprintln!("No clients connected. Closing connection to Pico.");
                opt_stream = None; // Ferme proprement le socket TCP
            }
            sleep(Duration::from_millis(500)).await;
            continue;
        }

        // 2. When there are clients, we need to ensure we have a connection to the Pico
        if opt_stream.is_none() {
            match tokio::net::TcpStream::connect(&car_addr).await {
                Ok(s) => {
                    eprintln!("Connected to car camera stream at {car_addr}");
                    let reader = BufReader::new(s);
                    opt_stream = Some(reader);
                }
                Err(e) => {
                    eprintln!("Failed to connect to car camera at {car_addr}: {e}");
                    sleep(Duration::from_secs(1)).await; // Attente avant reconnexion
                    continue;
                }
            }
        }

        // 3. Read and broadcast images
        if let Some(ref mut stream) = opt_stream {
            match tokio::time::timeout(Duration::from_secs(2), read_jpeg_from_stream(stream)).await {
                Ok(Ok(frame)) => {
                    let _ = tx.send(Arc::new(frame));
                }
                Ok(Err(e)) => {
                    eprintln!("Pico camera connection lost (read error): {e}. Attempting to reconnect...");
                    opt_stream = None;
                }
                Err(_timeout_error) => {
                    eprintln!("Pico camera connection timed out (no data received). Reconnecting...");
                    opt_stream = None; // triggers reconnection
                }
            }
        }
    }
}
