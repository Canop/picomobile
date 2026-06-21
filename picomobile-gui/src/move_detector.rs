use {
    image::GrayImage,
    std::{
        collections::VecDeque,
        sync::Arc,
        time::{
            Duration,
            Instant,
        },
    },
    tokio::sync::{
        broadcast,
        mpsc,
    },
};

const TERMINATE_WHEN_NO_RECEIVERS: bool = true;

#[derive(Debug, Clone, Copy)]
pub struct DetectionEvent {}

/// Configuration constants for tuning detection and performance
const DECODE_EVERY_N_FRAMES: usize = 5; // Sub-sampling rate
const STREAM_GAP_THRESHOLD: Duration = Duration::from_secs(2); // Detects stream stops/restarts
const PIXEL_THRESHOLD: u8 = 25; // Minimum luminance change to count as change
const MIN_TRIGGER_PERCENT: usize = 3; // Minimum percentage of changed pixels to trigger detection
const MAX_TRIGGER_PERCENT: usize = 40; // Cap to ignore sudden room lighting changes
const MIN_KEEP_PERCENT: usize = 1; // Minimum percentage to keep motion state active

pub async fn move_detector_task(
    mut rx: broadcast::Receiver<Arc<Vec<u8>>>,
    tx: mpsc::Sender<DetectionEvent>,
) {
    eprintln!("Move detector task started.");
    // Stores the last 2 processed GrayImages for 3-frame differencing
    let mut history: VecDeque<GrayImage> = VecDeque::with_capacity(2);
    let mut frame_count: usize = 0;
    let mut last_recv_time = Instant::now();
    let mut move_in_progress = false;

    loop {
        match rx.recv().await {
            Ok(jpeg_bytes) => {
                let now = Instant::now();

                // Requirement: Handle stream gaps and restarts.
                // If the time since the last frame exceeds the threshold, reset buffers.
                if now.duration_since(last_recv_time) > STREAM_GAP_THRESHOLD {
                    history.clear();
                    frame_count = 0;
                }
                last_recv_time = now;

                // Requirement: Sub-sampling to avoid heavy JPEG decoding load.
                frame_count += 1;
                if !frame_count.is_multiple_of(DECODE_EVERY_N_FRAMES) {
                    continue;
                }

                // Decode the JPEG payload only for sampled frames
                let jpeg_bytes_clone = jpeg_bytes.clone();
                let decode_result = tokio::task::spawn_blocking(move || {
                    image::load_from_memory(&jpeg_bytes_clone).map(|img| img.into_luma8())
                })
                .await;
                let current_frame = match decode_result {
                    Ok(Ok(frame)) => frame,
                    _ => continue, // Erreur de décodage ou jointure avortée
                };

                // Seed the history buffer before attempting comparison
                if history.len() < 2 {
                    history.push_back(current_frame);
                    move_in_progress = false;
                    continue;
                }

                // Assign frames for Three-Frame Differencing
                let oldest = &history[0];
                let middle = &history[1];
                let current = &current_frame;

                let (width, height) = current.dimensions();
                if width < 5 || height < 5 {
                    // Ignore frames that are too small to be meaningful
                    eprintln!("Move detector: frame too small ({}x{})", width, height);
                    continue;
                }
                let mut changed_pixels = 0;
                let total_pixels = (width * height) as usize;

                // Compute pixel-level intersection
                for x in 0..width {
                    for y in 0..height {
                        let p_old = oldest.get_pixel(x, y)[0];
                        let p_mid = middle.get_pixel(x, y)[0];
                        let p_cur = current.get_pixel(x, y)[0];

                        let diff1 = p_cur.abs_diff(p_mid);
                        let diff2 = p_mid.abs_diff(p_old);

                        // Pixel moved consistently across both temporal steps
                        if diff1 > PIXEL_THRESHOLD && diff2 > PIXEL_THRESHOLD {
                            changed_pixels += 1;
                        }
                    }
                }
                let change_percent = (changed_pixels * 100) / total_pixels;

                // Cycle the ring buffer history
                history.pop_front();
                history.push_back(current_frame);

                if move_in_progress {
                    if change_percent < MIN_KEEP_PERCENT {
                        move_in_progress = false;
                        eprintln!("Move detector: motion ended (change = {change_percent}%)");
                    }
                } else {
                    if change_percent >= MIN_TRIGGER_PERCENT {
                        if change_percent > MAX_TRIGGER_PERCENT {
                            eprintln!(
                                "Move detector: change = {change_percent}% (ignored, too high)"
                            );
                        } else {
                            eprintln!("Move detector: motion started (change = {change_percent}%)");
                            move_in_progress = true;
                            let event = DetectionEvent {};
                            match tx.try_send(event) {
                                Ok(_) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                    eprintln!("BIP! Motion detected!");
                                }
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                    if TERMINATE_WHEN_NO_RECEIVERS {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Err(broadcast::error::RecvError::Lagged(_)) => {
                // The task fell behind processing frames. Clear history to
                // ensure future comparisons aren't bound to stale time gaps.
                eprintln!("move detector lagged");
                history.clear();
                frame_count = 0;
                move_in_progress = false;
            }
            Err(broadcast::error::RecvError::Closed) => {
                eprintln!("move detector: broadcast channel closed");
                // Broadcast sender was dropped; terminate task.
                break;
            }
        }
    }
    eprintln!("Move detector task terminated.");
}
