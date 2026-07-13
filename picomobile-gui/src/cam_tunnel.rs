use {
    crate::*,
    jiff::Timestamp,
    std::{
        sync::Arc,
        time::Duration,
    },
    tokio::{
        io::{
            AsyncWriteExt,
            BufReader,
        },
        net::TcpStream,
        sync::broadcast,
        time::sleep,
    },
};

struct Stream {
    started: Timestamp,
    reader: BufReader<TcpStream>,
    frame_count: usize,
}

impl Stream {
    fn new(tcp_stream: TcpStream) -> Self {
        Self {
            started: Timestamp::now(),
            reader: BufReader::new(tcp_stream),
            frame_count: 0,
        }
    }
    fn fps(&self) -> f32 {
        let elapsed = Timestamp::now().duration_since(self.started).as_secs_f32();
        if elapsed > 0.0 {
            self.frame_count as f32 / elapsed
        } else {
            0.0
        }
    }
}

/// Fetches images from the car's camera and broadcasts them to all connected clients.
pub async fn camera_fetcher_task(
    car_addr: String,
    config_rx: watch::Receiver<CamConfig>,
    tx: broadcast::Sender<Arc<Vec<u8>>>,
    minimal_receivers_for_connection: usize,
) {
    let mut stream: Option<Stream> = None;

    loop {
        // If there's no client, we don't need to connect to the Pico
        if tx.receiver_count() < minimal_receivers_for_connection {
            if let Some(s) = stream.take() {
                eprintln!("Not enough subscribers. Closing connection to Pico.");
                eprintln!(
                    "Ending a stream of {} frames at {:.2} fps",
                    s.frame_count,
                    s.fps()
                );
            }
            sleep(Duration::from_millis(500)).await;
            continue;
        }

        // When there are clients, we need to ensure we have a connection to the Pico
        if stream.is_none() {
            match tokio::net::TcpStream::connect(&car_addr).await {
                Ok(mut s) => {
                    eprintln!("Connected to car camera stream at {car_addr}");
                    let config = *config_rx.borrow();
                    let resolution = config.resolution.to_string();
                    if let Err(e) = s.write_all(resolution.as_bytes()).await {
                        eprintln!("Failed to send resolution request to car camera: {e}");
                        continue;
                    }
                    stream = Some(Stream::new(s));
                }
                Err(e) => {
                    eprintln!("Failed to connect to car camera at {car_addr}: {e}");
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }
        }

        // Read and broadcast images
        if let Some(ref mut s) = stream {
            match tokio::time::timeout(Duration::from_secs(2), read_jpeg_from_stream(&mut s.reader))
                .await
            {
                Ok(Ok(frame)) => {
                    s.frame_count += 1;
                    if let Err(e) = tx.send(Arc::new(frame)) {
                        eprintln!("Failed to broadcast frame: {e}");
                    }
                }
                Ok(Err(e)) => {
                    eprintln!(
                        "Pico camera connection lost (read error): {e}. Attempting to reconnect..."
                    );
                    stream = None;
                }
                Err(_timeout_error) => {
                    eprintln!(
                        "Pico camera connection timed out (no data received). Reconnecting..."
                    );
                    stream = None; // triggers reconnection
                }
            }
        }
    }
}
