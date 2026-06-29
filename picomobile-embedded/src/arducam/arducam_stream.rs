use {
    crate::*,
    embassy_futures::yield_now,
    embassy_net::{
        Stack,
        tcp::TcpSocket,
    },
    embedded_io_async::Write,
    log::{
        error,
        info,
        warn,
    },
};

const WAIT_BETWEEN_FRAMES_MS: u64 = 100;

#[embassy_executor::task]
pub async fn camera_streaming_task(
    stack: Stack<'static>, // net stack
    mut arducam: Arducam<'static>,
) {
    let mut rx_buffer = [0u8; 512]; // TCP buffer for receiving commands from client
    let mut tx_buffer = [0u8; 8192]; // TCP buffer for sending the image to client
    let mut chunk_buffer = [0u8; 2048]; // buffer for the JPEG chunks read from SPI
    let mut buf = [0; 4096]; // buffer for reading commands from the TCP socket

    let mut current_resolution = None;

    if let Err(e) = arducam.init().await {
        log::error!("Arducam initialization failed: {e}");
        return;
    }
    info!("Arducam initialized successfully.");

    let resolution = Resolution::R640x480;
    if let Err(e) = arducam.set_resolution(resolution).await {
        log::error!("Failed to set Arducam resolution: {e}");
        return;
    }

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(5)));

        info!("Video stream: Listening on TCP port 1235...");
        if let Err(e) = socket.accept(1235).await {
            warn!("Error accept video stream: {:?}", e);
            continue;
        }

        info!(
            "Client connected to video stream from {:?}",
            socket.remote_endpoint()
        );

        let n = match socket.read(&mut buf).await {
            Ok(0) => {
                warn!("read EOF");
                break;
            }
            Ok(n) => n,
            Err(e) => {
                warn!("read error: {:?}", e);
                break;
            }
        };
        let Ok(input) = from_utf8(&buf[..n]) else {
            warn!("Received non-UTF8 data");
            continue;
        };
        info!("Received command: {}", input.trim());
        let resolution = match input.trim().parse::<Resolution>() {
            Ok(res) => res,
            Err(e) => {
                warn!("Invalid resolution command: {} (using default)", e);
                Resolution::default()
            }
        };
        if current_resolution != Some(resolution) {
            if let Err(e) = arducam.set_resolution(resolution).await {
                error!("Failed to set Arducam resolution: {e}");
                continue;
            }
            info!("Resolution set to {:?}", resolution);
            current_resolution = Some(resolution);
        }

        loop {
            // 1. order the Arducam to capture an image
            if let Err(e) = arducam.trigger_capture().await {
                log::error!("Error triggering Arducam capture: {e}");
                break;
            }

            // 2. Get the length of the image buffer
            // Note that this isn't the actual length of the JPEG image.
            // The arducam sends a bunch of 0xA5 after the JPEG data so
            // we'll also have to detect the end of the JPEG image by
            // looking for the EOI marker (0xFFD9).
            let mut total_bytes = arducam.get_fifo_length().await;
            let mut eoi_found = false;

            // 3. Start SPI burst read
            arducam.cs.set_low();
            if arducam.spi.write(&[BURST_FIFO_READ]).await.is_err() {
                arducam.cs.set_high();
                break;
            }

            // 4. As we can't fit the entire image in memory, we'll read
            // it in chunks and send the chunks over TCP (stripping the 0xA5
            // bytes at the end).
            let mut network_error = false;
            while total_bytes > 0 && !eoi_found {
                let to_read = core::cmp::min(total_bytes, chunk_buffer.len() as u32) as usize;

                // Reading the chunk from SPI into the temporary buffer
                if arducam
                    .spi
                    .transfer_in_place(&mut chunk_buffer[..to_read])
                    .await
                    .is_err()
                {
                    warn!("SPI read error during image transfer.");
                    network_error = true;
                    break;
                }

                let mut to_send = to_read;
                for i in 1..to_read {
                    if chunk_buffer[i - 1] == 0xFF && chunk_buffer[i] == 0xD9 {
                        to_send = i + 1; // cutting after the 0xD9
                        eoi_found = true;
                        break;
                    }
                }

                // Push the chunk to the TCP socket
                if let Err(e) = socket.write_all(&chunk_buffer[..to_send]).await {
                    warn!("Client disconnected during image transfer: {:?}", e);
                    warn!(
                        "to_sent: {}, to_read: {}, total_bytes: {}",
                        to_send, to_read, total_bytes
                    );
                    network_error = true;
                    break;
                }
                //info!("Sent {} bytes to client.", to_send);

                total_bytes -= to_read as u32;
                yield_now().await; // Yield to allow other tasks to run
            }

            // Release the SPI chip select line after the burst read
            arducam.cs.set_high();

            if network_error {
                break;
            }

            // Timerate reduction
            Timer::after_millis(WAIT_BETWEEN_FRAMES_MS).await;
        }

        socket.close();
        info!("Client disconnected from video stream.");
    }
}
