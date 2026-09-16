use std::{error::Error};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};

async fn handle_connection(mut stream: TcpStream) -> Result<(), Box<dyn Error>>{
    let mut buf   = [0; 1024];
    loop {
        let bytes_read = stream.read(&mut buf).await.unwrap();
        if bytes_read == 0 {
            break;
        }
        stream.write_all("+PONG\r\n".as_bytes()).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                eprintln!("Error reading from client: {}", e);
            }
        });
    }
}
