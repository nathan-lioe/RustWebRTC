use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

type PeerMap = Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>;

#[tokio::main]
async fn main() {
    let peers = PeerMap::default();
    let routes = warp::path("signaling")
        .and(warp::ws())
        .and(warp::any().map(move || Arc::clone(&peers)))
        .map(|ws: warp::ws::Ws, peers| {
            ws.on_upgrade(move |socket| handle_signaling(socket, peers))
        });

    println!("Signaling server running on ws://127.0.0.1:3030/signaling");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum SignalingMessage {
    Offer { sdp: RTCSessionDescription },
    Answer { sdp: RTCSessionDescription },
    Candidate { candidate: RTCIceCandidateInit },
}

async fn handle_signaling(ws: WebSocket, peers: PeerMap) {
    let (ws_tx, mut ws_rx) = ws.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx)); // Wrap ws_tx in Arc<Mutex<...>>
    let peer_connection = create_peer_connection().await.unwrap();
    let connection_id = uuid::Uuid::new_v4().to_string();

    // Insert peer connection into the map
    {
        let mut peers = peers.lock().await;
        peers.insert(connection_id.clone(), Arc::clone(&peer_connection));
    }

    setup_ice_candidates(Arc::clone(&peer_connection), Arc::clone(&ws_tx)).await;
    setup_tracks(Arc::clone(&peer_connection)).await;

    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str() {
                    if let Ok(signaling_message) = serde_json::from_str::<SignalingMessage>(text) {
                        match signaling_message {
                            SignalingMessage::Offer { sdp } => {
                                peer_connection.set_remote_description(sdp).await.unwrap();
                                let answer = peer_connection.create_answer(None).await.unwrap();
                                peer_connection
                                    .set_local_description(answer.clone())
                                    .await
                                    .unwrap();
                                let response = serde_json::to_string(&SignalingMessage::Answer {
                                    sdp: answer,
                                })
                                .unwrap();
                                ws_tx
                                    .lock()
                                    .await
                                    .send(Message::text(response))
                                    .await
                                    .unwrap();
                            }
                            SignalingMessage::Answer { sdp } => {
                                peer_connection.set_remote_description(sdp).await.unwrap();
                            }
                            SignalingMessage::Candidate { candidate } => {
                                peer_connection.add_ice_candidate(candidate).await.unwrap();
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving WebSocket message: {}", e);
                break;
            }
        }
    }

    println!("WebSocket connection closed");

    // Remove the peer connection from the map
    {
        let mut peers = peers.lock().await;
        peers.remove(&connection_id);
    }
}

// Create a new PeerConnection
async fn create_peer_connection() -> Result<Arc<RTCPeerConnection>, webrtc::Error> {
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs().unwrap();

    let api = APIBuilder::new().with_media_engine(media_engine).build();
    let config = RTCConfiguration::default();

    Ok(Arc::new(api.new_peer_connection(config).await?))
}

async fn setup_ice_candidates(
    peer_connection: Arc<RTCPeerConnection>,
    ws_tx: Arc<Mutex<SplitSink<WebSocket, Message>>>, // Update ws_tx to Arc<Mutex<SplitSink<...>>>
) {
    peer_connection.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
        let ws_tx = Arc::clone(&ws_tx);
        Box::pin(async move {
            if let Some(candidate) = candidate {
                let candidate_init = candidate.to_json().unwrap();
                let msg = serde_json::to_string(&SignalingMessage::Candidate {
                    candidate: candidate_init,
                })
                .unwrap();
                // Lock ws_tx and send the message
                ws_tx.lock().await.send(Message::text(msg)).await.unwrap();
            }
        })
    }));
}

async fn setup_tracks(peer_connection: Arc<RTCPeerConnection>) {
    peer_connection.on_track(Box::new(|track, _| {
        println!("New track received: {:?}", track);
        Box::pin(async {})
    }));
}
