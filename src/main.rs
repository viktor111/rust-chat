use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::StreamExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};
use std::error::Error;
use tokio_util::codec::{Framed, LinesCodec};
use futures::sink::SinkExt;

//todos:
// 1. refactor process method
// 2. add password functionality
// 3. add help command

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>>{

    let state = Arc::new(Mutex::new(Shared::new()));

    let addr = "127.0.0.1:4000";
    
    let mut listener = TcpListener::bind(&addr).await?;

    println!("Listening on: {}", addr);

    loop{
        let (stream, addr) = listener.accept().await?;

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            println!("Accepted connection from");
            if let Err(e) = process(state, stream, addr).await {
                println!("failed to process connection; error = {:?}", e);
            };
        });
    }
}

type Tx = mpsc::UnboundedSender<String>;
type Rx = mpsc::UnboundedReceiver<String>;

struct Shared {
    peers: HashMap<SocketAddr, Tx>,
    usernames: HashMap<SocketAddr, String>,
}

struct Peer {
   rx: Rx,
   lines: Framed<TcpStream, LinesCodec>,
}

impl Shared{
    fn new() -> Self{
        Self{
            peers: HashMap::new(),
            usernames: HashMap::new(),
        }
    }

    async fn add_username(&mut self, username: String, addr: SocketAddr){
        self.usernames.insert(addr, username);
    }

    async fn remove_username(&mut self, addr: SocketAddr){
        self.usernames.remove(&addr);
    }

    async fn check_username(&mut self, username_to_check: &str) -> bool{
        for (addr, username) in &self.usernames{
            if username == username_to_check{
                return true;
            }
        }

        return false;
    }

    async fn print_usernames(&mut self){
        for (addr, username) in &self.usernames{
            println!("{}: {}", addr, username);
        }
    }

    async fn broadcast(&mut self, sender: SocketAddr, message: &str){
        for peer in self.peers.iter_mut() {
            println!("Sending message to {}", peer.0);
            if *peer.0 != sender {
                let _ = peer.1.send(message.into());
            }
        }
    }
}

impl Peer{
    async fn new(state: Arc<Mutex<Shared>>, lines: Framed<TcpStream, LinesCodec>) -> io::Result<Peer>{
        let addr = lines.get_ref().peer_addr()?;

        let (tx, rx) = mpsc::unbounded_channel();

        state.lock().await.peers.insert(addr, tx);

        Ok(Self{
            rx,
            lines,
        })
    }
}

async fn process(state: Arc<Mutex<Shared>>, stream: TcpStream, addr: SocketAddr) -> Result<(), Box<dyn Error>>{
    let mut lines = Framed::new(stream, LinesCodec::new());

    loop {
        lines.send("Enter username:").await?; 

        let username_input = match lines.next().await {
            Some(Ok(line)) => {
                line
            },
            _  => {
                return Ok(());
            }
        }; 

        let is_username_taken = state.lock().await.check_username(&username_input).await;
        println!("Is username taken: {}", is_username_taken);

        if is_username_taken {
            lines.send("Username is taken try again").await?;
            println!("[-] Username is taken try again");
        }
        else{
            state.lock().await.add_username(username_input, addr).await;
            println!("[+] Username added");
            state.lock().await.print_usernames().await;
            break;
        }

        lines.send("Enter new username:").await?;
    }

    let mut peer = Peer::new(state.clone(), lines).await?;

    {
        let mut state = state.lock().await;
        let msg = format!("{} has joined the chat", state.usernames.get(&addr).unwrap());
        state.broadcast(addr, &msg).await;
    }

    loop {
        tokio::select! {
            Some(msg) = peer.rx.recv() => {
                peer.lines.send(&msg).await?;
            }
            result = peer.lines.next() => match result {
                Some(Ok(msg)) => {
                    let mut state = state.lock().await;
                    let username = state.usernames.get(&addr).unwrap();
                    let msg = format!("{}: {}", username, msg);

                    state.broadcast(addr, &msg).await;
                }
                Some(Err(e)) => {
                    println!("failed to read from socket; err = {:?}", e);
                }
                None => break,
            },
        }
    }

    {
        let mut state = state.lock().await;
        state.peers.remove(&addr);
        let username = state.usernames.get(&addr).unwrap();
        let msg = format!("{} has left the chat", username);
        println!("{}", msg);
        state.broadcast(addr, &msg).await;
    }

    Ok(())
}