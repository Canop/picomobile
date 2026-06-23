use {
    crate::*,
    image::GrayImage,
    jiff::Timestamp,
    std::{
        collections::VecDeque,
        sync::Arc,
        time::{
            Duration,
            Instant,
        },
    },
    tokio::sync::broadcast,
};

#[derive(Debug, Clone)]
pub struct DetectionEvent {
    pub time: Timestamp,
    pub images: Vec<Arc<Vec<u8>>>,
}

/// Configuration constants for tuning detection and performance
const DECODE_EVERY_N_FRAMES: usize = 1; // Sub-sampling rate
const STREAM_GAP_THRESHOLD: Duration = Duration::from_secs(2); // Detects stream stops/restarts
const PIXEL_THRESHOLD: u8 = 25; // Minimum luminance change to count as change
const MIN_TRIGGER_PERCENT: f32 = 0.1; // Minimum percentage of changed pixels to trigger detection
const MAX_TRIGGER_PERCENT: f32 = 30.0; // Cap to ignore sudden lighting changes
const MIN_KEEP_PERCENT: f32 = 0.05; // Minimum percentage to keep motion state active
const MIN_EVENT_IMAGES: usize = 2; // filters out events too short to be meaningful
const MAX_EVENT_IMAGES: usize = 10; // Max images to store per event to limit memory usage

pub async fn move_detector_task(
    mut rx: broadcast::Receiver<Arc<Vec<u8>>>,
    config_rx: watch::Receiver<MotionDetectionConfig>,
    tx: broadcast::Sender<DetectionEvent>,
) {
    eprintln!("Move detector task started.");
    // Stores the last 2 processed GrayImages for 3-frame differencing
    let mut history: VecDeque<GrayImage> = VecDeque::with_capacity(2);
    let mut frame_count: usize = 0;
    let mut last_recv_time = Instant::now();
    let mut event: Option<DetectionEvent> = None;

    loop {
        match rx.recv().await {
            Ok(jpeg_bytes) => {
                let config = *config_rx.borrow();
                if !config.enable_motion_detection {
                    history.clear();
                    frame_count = 0;
                    continue;
                }

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
                    event = None; // Reset any ongoing event since we don't have enough frames yet
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
                let change_percent = (changed_pixels as f32 * 100.0) / total_pixels as f32;

                // Cycle the ring buffer history
                history.pop_front();
                history.push_back(current_frame);

                match event.take() {
                    Some(mut e) => {
                        if change_percent < MIN_KEEP_PERCENT {
                            eprintln!(
                                "Move detector: motion ended (change = {change_percent:.2}%)"
                            );
                            if e.images.len() < MIN_EVENT_IMAGES {
                                eprintln!(
                                    "Move detector: event ignored (only {} images)",
                                    e.images.len()
                                );
                            } else {
                                match tx.send(e) {
                                    Ok(_) => {
                                        eprintln!("Move detector: event sent to receivers");
                                    }
                                    Err(_) => {
                                        eprintln!("Move detector: no receivers -> closing task");
                                        break;
                                    }
                                }
                            }
                        } else {
                            eprintln!(
                                "Move detector: motion ongoing (change = {change_percent:.2}%)"
                            );
                            if e.images.len() < MAX_EVENT_IMAGES {
                                e.images.push(jpeg_bytes.clone());
                            }
                            event = Some(e);
                        }
                    }
                    None => {
                        if change_percent > 0.03 {
                            eprintln!("Move detector: change = {change_percent:.2}%");
                        }
                        if change_percent >= MIN_TRIGGER_PERCENT {
                            if change_percent > MAX_TRIGGER_PERCENT {
                                eprintln!(
                                    "Move detector: change = {change_percent:.2}% (ignored, too high)"
                                );
                            } else {
                                eprintln!(
                                    "Move detector: motion started (change = {change_percent:.2}%)"
                                );
                                event = Some(DetectionEvent {
                                    time: Timestamp::now(),
                                    images: vec![jpeg_bytes.clone()],
                                });
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
                event = None; // Reset any ongoing event since we lost frames
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
