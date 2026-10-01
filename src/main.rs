use std::{error::Error, sync::{Arc, Mutex}};
use bytes::{BytesMut, Buf};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};

mod parser;
mod executor;
mod data_manager;

const INITIAL_CAPACITY: usize = 1024;

async fn handle_connection(mut stream: TcpStream, db: Arc<Mutex<data_manager::DataManager>>) -> Result<(), Box<dyn Error>> {
    let mut buffer = BytesMut::with_capacity(INITIAL_CAPACITY);
    loop {
        let bytes_read = stream.read_buf(&mut buffer).await?;
        if buffer.len() > parser::MAX_BULK_LEN as usize {
            stream.write_all(b"-ERR invalid request\r\n").await?;
            break;
        }
        if bytes_read == 0 {
            break;
        }
        while !buffer.is_empty() {
            match parser::parse(&buffer) {
                Ok((resp_value, bytes_consumed)) => {
                    buffer.advance(bytes_consumed);
                    let res = executor::execute(resp_value, &db);
                    stream.write_all(&res.encode()).await?;
                }
                Err(err) => match err {
                    parser::ParserError::Incomplete => {
                        break;
                    },
                    parser::ParserError::Invalid(msg) => {
                        eprintln!("Parsing error: {}", msg);
                        stream.write_all(b"-ERR invalid request\r\n").await?;
                        return Ok(());
                    },
                },
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
