use {
    std::path::Path,
    tokio::{
        fs::{
            self,
            File,
        },
        io::{
            AsyncReadExt,
            AsyncWriteExt,
            BufReader,
        },
        net::TcpStream,
    },
};

/// Reads a JPEG image from a TCP stream until the EOI (End of Image) marker is found.
pub async fn read_jpeg_from_stream_into(
    stream: &mut BufReader<TcpStream>,
    img_buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    img_buf.clear();
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte).await?;
        img_buf.push(byte[0]);
        // Detection of the EOI (End of Image) flag : 0xFF 0xD9
        if img_buf.len() >= 2
            && img_buf[img_buf.len() - 2] == 0xFF
            && img_buf[img_buf.len() - 1] == 0xD9
        {
            break;
        }
    }
    Ok(())
}
/// Reads a JPEG image from a TCP stream until the EOI (End of Image) marker is found.
pub async fn read_jpeg_from_stream(stream: &mut BufReader<TcpStream>) -> anyhow::Result<Vec<u8>> {
    let mut img_buf = Vec::with_capacity(128 * 1024);
    read_jpeg_from_stream_into(stream, &mut img_buf).await?;
    Ok(img_buf)
}

/// A simple function to capture a few images from the camera and
/// save them to a directory (mostly for testing purposes).
pub async fn write_cam_images<P: AsRef<Path>>(
    cam_addr: String,
    dir_path: P,
    n: usize, // number of images to capture
) -> anyhow::Result<()> {
    fs::create_dir_all(&dir_path).await?;

    eprintln!("Connect the camera on {}...", cam_addr);
    let stream = TcpStream::connect(cam_addr).await?;

    eprintln!("Connected. Taking {} images...", n);
    let mut reader = BufReader::new(stream);
    let mut count = 0;
    let mut img_buf = Vec::with_capacity(128 * 1024);

    while count < n {
        read_jpeg_from_stream_into(&mut reader, &mut img_buf).await?;
        count += 1;
        let filename = dir_path.as_ref().join(format!("pic-{}.jpg", count));
        let mut file = File::create(&filename).await?;
        file.write_all(&img_buf).await?;
        file.flush().await?;
        eprintln!("Image {}/{} saved : {:?}", count, n, filename);
    }
    eprintln!("End of capture. {} images saved.", count);

    Ok(())
}
