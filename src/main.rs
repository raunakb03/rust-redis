use std::{error::Error, sync::{Arc, Mutex}};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};

mod parser;
mod executor;
mod data_manager;

async fn handle_connection(mut stream: TcpStream, db: Arc<Mutex<data_manager::DataManager>>) -> Result<(), Box<dyn Error>>{
    let mut buf = [0; 1024];
    loop {
        let bytes_read = stream.read(&mut buf).await?;
        if bytes_read == 0 {
            break;
        }
        
        let mut start = 0;
        while start < bytes_read {
            if let Some((parsed_req, new_start)) = parser::parse(start, bytes_read - 1, &buf[0..bytes_read]) {
                println!("Parsed request: {:?}", parsed_req);
                let res = executor::execute(parsed_req, &db);
                stream.write_all(res.as_bytes()).await?;
                start = new_start;
            } else {
                if start == 0 {
                    stream.write_all(b"-ERR invalid request\r\n").await?;
                }
                break;
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    let db = Arc::new(Mutex::new(data_manager::DataManager::new()));

    loop {
        let (socket, _) = listener.accept().await?;
        let db_clone = Arc::clone(&db);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, db_clone).await {
                eprintln!("Error reading from client: {}", e);
            }
        });
    }
}
