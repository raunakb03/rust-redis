use std::{error::Error};
use tokio::{io::AsyncWriteExt, net::{TcpListener, TcpStream}};

async fn handle_connection(mut stream: TcpStream) -> Result<(), Box<dyn Error>>{
    stream.write_all("+PONG\r\n".as_bytes()).await?;
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
