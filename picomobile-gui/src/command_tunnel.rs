use {
    std::time::Duration,
    tokio::{
        io::AsyncWriteExt,
        net::TcpStream,
        sync::mpsc,
        time::sleep,
    },
};

pub async fn tunnel_commands_task(
    car_commands_addr: String,
    mut rx: mpsc::Receiver<String>,
) {
    let mut stream: Option<TcpStream> = None;

    while let Some(command) = rx.recv().await {
        let payload = format!("{command}\n");

        loop {
            if stream.is_none() {
                match TcpStream::connect(&car_commands_addr).await {
                    Ok(s) => {
                        println!("Connected to car control port");
                        stream = Some(s);
                    }
                    Err(e) => {
                        eprintln!("Failed to connect to car on {car_commands_addr:?}: {e}");
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
                    eprintln!("Connection lost while sending '{}': {}", command, e);
                    stream = None;
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
    println!("Writer task exiting");
}
