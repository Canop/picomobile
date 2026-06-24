mod args;
mod cam_capture;
mod cam_tunnel;
mod command_tunnel;
mod config;
mod event_saver;
mod move_detector;
mod pico_ports;
mod sound;
mod resolution;

pub use {
    args::*,
    cam_capture::*,
    command_tunnel::*,
    config::*,
    event_saver::*,
    move_detector::*,
    pico_ports::*,
    sound::*,
    resolution::*,
};

use {
    axum::{
        Json,
        Router,
        extract::State,
        http::StatusCode,
        response::{
            IntoResponse,
            Response,
        },
        routing::get,
        routing::post,
    },
    clap::Parser,
    serde::Deserialize,
    std::{
        net::SocketAddr,
        sync::Arc,
    },
    tokio::sync::{
        broadcast,
        mpsc,
        watch,
    },
    tokio_stream::StreamExt,
    tokio_stream::wrappers::{
        BroadcastStream,
        errors::BroadcastStreamRecvError,
    },
    tower_http::services::ServeDir,
};

/// Ports used for communication with the Pico
pub const PICO_PORTS: PicoPorts = PicoPorts {
    command_port: 1234,
    image_port: 1235,
};

#[derive(Clone)]
struct AppState {
    cam_config_tx: watch::Sender<CamConfig>,
    cam_config_rx: watch::Receiver<CamConfig>,
    command_tx: mpsc::Sender<String>,
    video_tx: broadcast::Sender<Arc<Vec<u8>>>,
}

#[derive(Deserialize)]
struct CommandRequest {
    command: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Some(dir_path) = args.capture_images.as_ref() {
        let cam_addr = format!("{}:{}", args.car_ip, PICO_PORTS.image_port);
        if let Err(e) = cam_capture::write_cam_images(cam_addr, dir_path, 5).await {
            eprintln!("Error capturing images: {}", e);
        }
        return;
    }

    if let Some(gui_port) = args.gui_port {
        println!("Starting GUI on port {}", gui_port);
        serve(&args.car_ip, gui_port).await;
    }

    eprintln!("Not capturing, and not serving GUI. Exiting.");
}

async fn serve(
    car_ip: &str,
    gui_port: u16,
) {
    let cam_config = CamConfig::default();

    // command channel
    let (command_tx, command_rx) = mpsc::channel::<String>(8);
    // video channel
    let video_tx = broadcast::Sender::<Arc<Vec<u8>>>::new(64);
    // motion detection events channel
    let detection_tx = broadcast::Sender::<DetectionEvent>::new(1);
    // cam config updates channel
    let cam_config_tx = watch::Sender::<CamConfig>::new(cam_config);

    let car_commands_addr = format!("{car_ip}:{}", PICO_PORTS.command_port);
    tokio::spawn(tunnel_commands_task(car_commands_addr, command_rx));

    tokio::spawn(event_saver::event_saver_task(
        detection_tx.subscribe(),
        cam_config_tx.subscribe(),
    ));
    tokio::spawn(sound::sound_player_task(
        detection_tx.subscribe(),
        cam_config_tx.subscribe(),
    ));
    tokio::spawn(move_detector::move_detector_task(
        video_tx.subscribe(),
        cam_config_tx.subscribe(),
        detection_tx,
    ));

    let car_camera_addr = format!("{car_ip}:{}", PICO_PORTS.image_port);

    // Logic behind fetching image from the Pico is we do it only when the browser asks for
    // images (and we don't want move detection to run when the browser is not connected, to avoid
    // unnecessary load on the Pico).
    let minimal_receivers_for_connection = 2; // move detector + at least one GUI client
    tokio::spawn(cam_tunnel::camera_fetcher_task(
        car_camera_addr,
        cam_config_tx.subscribe(),
        video_tx.clone(),
        minimal_receivers_for_connection,
    ));

    let state = AppState {
        cam_config_rx: cam_config_tx.subscribe(),
        cam_config_tx,
        command_tx,
        video_tx,
    };

    let app = Router::new()
        .route("/api/command", post(send_command))
        .route("/api/video", get(video_stream))
        .route("/api/cam-config", get(get_cam_config))
        .route("/api/cam-config", post(update_cam_config))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let gui_addr = SocketAddr::from(([127, 0, 0, 1], gui_port));

    println!("GUI available on http://{}", gui_addr);

    let listener = tokio::net::TcpListener::bind(gui_addr)
        .await
        .expect("bind failed");

    axum::serve(listener, app).await.expect("server failed");
}

/// endpoint receiving commands from the GUI and sending them to the car
async fn send_command(
    State(state): State<AppState>,
    Json(req): Json<CommandRequest>,
) -> impl IntoResponse {
    match state.command_tx.try_send(req.command) {
        Ok(_) => StatusCode::NO_CONTENT,

        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            // Queue already contains newer commands waiting to be sent.
            // Drop this one.
            StatusCode::NO_CONTENT
        }

        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

/// endpoint streaming MJPEG video from the car to the GUI
async fn video_stream(State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.video_tx.subscribe();
    let broadcast_stream = BroadcastStream::new(rx);

    let mjpeg_stream = broadcast_stream.filter_map(|result| match result {
        Ok(frame) => {
            let chunk = format!(
                "--boundary\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                frame.len()
            );
            let mut data = chunk.into_bytes();
            data.extend_from_slice(&frame);
            data.extend_from_slice(b"\r\n");
            Some(Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(
                data,
            )))
        }
        Err(BroadcastStreamRecvError::Lagged(_)) => None,
    });

    Response::builder()
        .header(
            "Content-Type",
            "multipart/x-mixed-replace; boundary=boundary",
        )
        .body(axum::body::Body::from_stream(mjpeg_stream))
        .unwrap()
}

async fn get_cam_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.cam_config_rx.borrow();
    Json(*config)
}

async fn update_cam_config(
    State(state): State<AppState>,
    Json(req): Json<UpdateCamConfig>,
) -> impl IntoResponse {
    state.cam_config_tx.send_modify(|c| {
        c.update(req);
    });
    eprintln!("Updated cam config: {:?}", *state.cam_config_tx.borrow());
    Json(*state.cam_config_tx.borrow())
}
