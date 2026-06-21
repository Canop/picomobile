mod args;
mod cam_capture;
mod cam_tunnel;
mod command_tunnel;
mod pico_ports;

pub use {
    args::*,
    cam_capture::*,
    command_tunnel::*,
    pico_ports::*,
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
        routing::post,
        routing::get,
    },
    clap::Parser,
    //futures_util::stream::Stream,
    serde::Deserialize,
    std::{
        net::SocketAddr,
        sync::Arc,
    },
    tokio::sync::{
        broadcast,
        mpsc,
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
    // command channel
    let (command_tx, command_rx) = mpsc::channel::<String>(8);
    // video channel
    let video_tx = broadcast::Sender::<Arc<Vec<u8>>>::new(16);

    let car_commands_addr = format!("{car_ip}:{}", PICO_PORTS.command_port);
    tokio::spawn(tunnel_commands_task(car_commands_addr, command_rx));

    let car_camera_addr = format!("{car_ip}:{}", PICO_PORTS.image_port);
    tokio::spawn(cam_tunnel::camera_fetcher_task(car_camera_addr, video_tx.clone()));

    let state = AppState { command_tx, video_tx };

    let app = Router::new()
        .route("/api/command", post(send_command))
        .route("/api/video", get(video_stream))
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

async fn video_stream(State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.video_tx.subscribe();
    let broadcast_stream = BroadcastStream::new(rx);

    // Transformation du broadcast en flux de chunks HTTP Multipart MJPEG
    let mjpeg_stream = broadcast_stream.map(|result| {
        match result {
            Ok(frame) => {
                let chunk = format!(
                    "--boundary\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    frame.len()
                );
                let mut data = chunk.into_bytes();
                data.extend_from_slice(&frame);
                data.extend_from_slice(b"\r\n");
                Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(data))
            }
            Err(BroadcastStreamRecvError::Lagged(_)) => {
                // Le client est trop lent, on ignore les frames perdues
                Ok(axum::body::Bytes::new())
            }
        }
    });

    Response::builder()
        .header(
            "Content-Type",
            "multipart/x-mixed-replace; boundary=boundary",
        )
        .body(axum::body::Body::from_stream(mjpeg_stream))
        .unwrap()
}
