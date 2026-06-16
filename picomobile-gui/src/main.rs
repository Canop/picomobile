use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use clap::Parser;
use serde::Deserialize;
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::mpsc,
    time::sleep,
};
use tower_http::services::ServeDir;

#[derive(Parser)]
struct Args {
    /// Car IP address
    #[arg(long)]
    car_ip: String,

    /// Car TCP port
    #[arg(long)]
    car_port: u16,

    /// Web GUI port
    #[arg(long)]
    gui_port: u16,
}

#[derive(Clone)]
struct AppState {
    command_tx: mpsc::Sender<String>,
}

#[derive(Deserialize)]
struct CommandRequest {
    command: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let car_addr = Arc::new(format!("{}:{}", args.car_ip, args.car_port));
    let (command_tx, command_rx) = mpsc::channel::<String>(8);

    tokio::spawn(writer_task(
        Arc::clone(&car_addr),
        command_rx,
    ));

    let state = AppState {
        command_tx,
    };

    let app = Router::new()
        .route("/api/command", post(send_command))
        .fallback_service(ServeDir::new("static"))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.gui_port));

    println!("GUI available on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind failed");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

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

        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

async fn writer_task(
    car_addr: Arc<String>,
    mut rx: mpsc::Receiver<String>,
) {
    let mut stream: Option<TcpStream> = None;

    while let Some(command) = rx.recv().await {
        let payload = format!("{command}\n");

        loop {
            if stream.is_none() {
                match TcpStream::connect(car_addr.as_str()).await {
                    Ok(s) => {
                        println!("Connected to car");
                        stream = Some(s);
                    }
                    Err(e) => {
                        eprintln!( "Failed to connect to car: {e}");
                        break;
                    }
                }
            }

            let write_result = match stream.as_mut() {
                Some(s) => s.write_all(payload.as_bytes()).await,
                None => unreachable!(),
            };

            match write_result {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    eprintln!(
                        "Connection lost while sending '{}': {}",
                        command,
                        e
                    );
                    stream = None;
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    println!("Writer task exiting");
}
