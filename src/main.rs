use env_logger;
use std::net::SocketAddr;
use std::sync::Arc;
use warp::Filter;

mod signaling; // Import the signaling module
use signaling::{handle_signaling, PeerMap};

// Define the server address as a static string
const SERVER_ADDRESS: &str = "127.0.0.1:3030";

#[tokio::main]
async fn main() {
    env_logger::init();

    let peers = PeerMap::default();
    let routes = warp::path("signaling")
        .and(warp::ws())
        .and(warp::any().map(move || Arc::clone(&peers)))
        .map(|ws: warp::ws::Ws, peers| {
            ws.on_upgrade(move |socket| handle_signaling(socket, peers))
        });

    // Parse the server address into a SocketAddr
    let addr: SocketAddr = SERVER_ADDRESS.parse().expect("Invalid server address");

    println!("Signaling server running on ws://{}", SERVER_ADDRESS);
    warp::serve(routes).run(addr).await;
}
