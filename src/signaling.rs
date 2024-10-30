use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::ws::{Message, WebSocket};
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

pub type PeerMap = Arc<Mutex<HashMap<String, Arc<RTCPeerConnection>>>>;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum SignalingMessage {
    Offer { sdp: RTCSessionDescription },
    Answer { sdp: RTCSessionDescription },
    Candidate { candidate: RTCIceCandidateInit },
}

pub async fn handle_signaling(ws: WebSocket, peers: PeerMap) {
    let (ws_tx, mut ws_rx) = ws.split();
    let ws_tx = Arc::new(Mutex::new(ws_tx));
    let peer_connection = match create_peer_connection().await {
        Ok(pc) => pc,
        Err(e) => {
            error!("Failed to create peer connection: {}", e);
            return; // Early exit if peer connection creation fails
        }
    };
    let connection_id = uuid::Uuid::new_v4().to_string();

    // Insert peer connection into the map
    {
        let mut peers = peers.lock().await;
        peers.insert(connection_id.clone(), Arc::clone(&peer_connection));
    }

    if let Err(e) = setup_ice_candidates(Arc::clone(&peer_connection), Arc::clone(&ws_tx)).await {
        error!("Failed to set up ICE candidates: {}", e);
        return;
    }
    if let Err(e) = setup_tracks(Arc::clone(&peer_connection)).await {
        error!("Failed to set up tracks: {}", e);
        return;
    }

    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str() {
                    match serde_json::from_str::<SignalingMessage>(text) {
                        Ok(signaling_message) => match signaling_message {
                            SignalingMessage::Offer { sdp } => {
                                if let Err(e) = peer_connection.set_remote_description(sdp).await {
                                    error!("Failed to set remote description: {}", e);
                                    continue;
                                }
                                let answer = match peer_connection.create_answer(None).await {
                                    Ok(ans) => ans,
                                    Err(e) => {
                                        error!("Failed to create answer: {}", e);
                                        continue;
                                    }
                                };
                                if let Err(e) =
                                    peer_connection.set_local_description(answer.clone()).await
                                {
                                    error!("Failed to set local description: {}", e);
                                    continue;
                                }
                                let response =
                                    match serde_json::to_string(&SignalingMessage::Answer {
                                        sdp: answer,
                                    }) {
                                        Ok(res) => res,
                                        Err(e) => {
                                            error!("Failed to serialize answer message: {}", e);
                                            continue;
                                        }
                                    };
                                if let Err(e) =
                                    ws_tx.lock().await.send(Message::text(response)).await
                                {
                                    error!("Failed to send answer message: {}", e);
                                }
                            }
                            SignalingMessage::Answer { sdp } => {
                                if let Err(e) = peer_connection.set_remote_description(sdp).await {
                                    error!("Failed to set remote description: {}", e);
                                }
                            }
                            SignalingMessage::Candidate { candidate } => {
                                if let Err(e) = peer_connection.add_ice_candidate(candidate).await {
                                    error!("Failed to add ICE candidate: {}", e);
                                }
                            }
                        },
                        Err(e) => {
                            error!("Failed to parse signaling message: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error receiving WebSocket message: {}", e);
                break;
            }
        }
    }

    info!("WebSocket connection closed");

    // Remove the peer connection from the map
    {
        let mut peers = peers.lock().await;
        peers.remove(&connection_id);
    }
}

async fn create_peer_connection() -> Result<Arc<RTCPeerConnection>, webrtc::Error> {
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs().unwrap();

    let api = APIBuilder::new().with_media_engine(media_engine).build();
    let config = RTCConfiguration::default();

    Ok(Arc::new(api.new_peer_connection(config).await?))
}

async fn setup_ice_candidates(
    peer_connection: Arc<RTCPeerConnection>,
    ws_tx: Arc<Mutex<SplitSink<WebSocket, Message>>>,
) -> Result<(), webrtc::Error> {
    peer_connection.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
        let ws_tx = Arc::clone(&ws_tx);
        Box::pin(async move {
            if let Some(candidate) = candidate {
                let candidate_init = candidate.to_json().unwrap();
                let msg = serde_json::to_string(&SignalingMessage::Candidate {
                    candidate: candidate_init,
                })
                .unwrap();
                if let Err(e) = ws_tx.lock().await.send(Message::text(msg)).await {
                    error!("Failed to send ICE candidate: {}", e);
                }
            }
        })
    }));

    Ok(())
}

async fn setup_tracks(peer_connection: Arc<RTCPeerConnection>) -> Result<(), webrtc::Error> {
    peer_connection.on_track(Box::new(|track, _| {
        info!("New track received: {:?}", track);
        Box::pin(async {})
    }));

    Ok(())
}
