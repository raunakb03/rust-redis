use std::{error::Error};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};

mod parser;
mod executor;

async fn handle_connection(mut stream: TcpStream) -> Result<(), Box<dyn Error>>{
    let mut buf = [0; 1024];
    loop {
        let bytes_read = stream.read(&mut buf).await?;
        if bytes_read == 0 {
            break;
        }
        let mut res = String::from("+PONG\r\n");
        if bytes_read > 0 && let Some((parsed_req, _)) = parser::parse(0, bytes_read - 1, &buf[0..bytes_read]) {
            println!("Parsed request: {:?}", parsed_req);
            res = executor::execute(parsed_req);
        }
        stream.write_all(res.as_bytes()).await?;
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
