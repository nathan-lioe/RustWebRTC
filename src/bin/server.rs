use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex; // Use tokio's async Mutex
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use warp::Filter;

type Peers = Arc<Mutex<HashMap<String, Arc<Mutex<SplitSink<WebSocket, Message>>>>>>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
enum SignalingMessage {
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    Candidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u32>,
    },
}

#[tokio::main]
async fn main() {
    let peers: Peers = Arc::new(Mutex::new(HashMap::new()));

    let signaling_route = warp::path("signaling")
        .and(warp::ws())
        .and(with_peers(peers.clone()))
        .map(|ws: warp::ws::Ws, peers| {
            ws.on_upgrade(move |socket| handle_connection(socket, peers))
        });

    println!("Signaling server running on ws://127.0.0.1:3030/signaling");
    warp::serve(signaling_route)
        .run(([127, 0, 0, 1], 3030))
        .await;
}

fn with_peers(
    peers: Peers,
) -> impl Filter<Extract = (Peers,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || peers.clone())
}

async fn handle_connection(ws: WebSocket, peers: Peers) {
    let (sender, mut receiver) = ws.split();
    let sender = Arc::new(Mutex::new(sender));

    let client_id = Uuid::new_v4().to_string();
    peers.lock().await.insert(client_id.clone(), sender.clone());

    println!("Client {} connected", client_id);

    while let Some(result) = receiver.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str() {
                    println!("Received message from {}: {}", client_id, text);

                    // Attempt to parse the message
                    let signaling_message: Result<SignalingMessage, _> = serde_json::from_str(text);
                    match signaling_message {
                        Ok(message) => {
                            println!("Parsed message successfully: {:?}", message);
                            forward_message(&client_id, &message, &peers).await;
                        }
                        Err(e) => {
                            eprintln!(
                                "Error parsing message from client {}: {} - Error: {:?}",
                                client_id, text, e
                            );
                            // Log and continue, but do not break the connection
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving message for client {}: {}", client_id, e);
                break;
            }
        }
    }

    peers.lock().await.remove(&client_id);
    println!("Client {} disconnected", client_id);
}

async fn forward_message(sender_id: &str, message: &SignalingMessage, peers: &Peers) {
    let serialized_message = match serde_json::to_string(message) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Failed to serialize message: {}", e);
            return;
        }
    };

    let peers = peers.lock().await; // Await the async Mutex lock
    for (client_id, client) in peers.iter() {
        if client_id != sender_id {
            let mut client = client.lock().await; // Await the async Mutex lock
            if let Err(e) = client.send(Message::text(serialized_message.clone())).await {
                eprintln!("Error sending message to {}: {}", client_id, e);
            }
        }
    }
}
