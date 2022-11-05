use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = TcpStream::connect("127.0.0.1:4000").await?;
    
    loop {
        let mut buffer = [0; 1024];
        connection.read(&mut buffer).await?;
        println!("{}", String::from_utf8_lossy(&buffer[..]));

        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        connection.write_all(line.as_bytes()).await?;
    }
}
