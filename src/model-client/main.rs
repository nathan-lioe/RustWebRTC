use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum SignalingMessage {
    Offer { sdp: RTCSessionDescription },
    Answer { sdp: RTCSessionDescription },
    Candidate { candidate: RTCIceCandidateInit },
}

struct VideoStreamer {
    peer_connection: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
}

impl VideoStreamer {
    async fn new() -> Result<Self> {
        let mut media_engine = MediaEngine::default();
        media_engine
            .register_default_codecs()
            .context("Failed to register codecs")?;

        let api = APIBuilder::new().with_media_engine(media_engine).build();

        let config = RTCConfiguration::default();
        let peer_connection = Arc::new(
            api.new_peer_connection(config)
                .await
                .context("Failed to create peer connection")?,
        );

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: "video/h264".to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));

        Ok(VideoStreamer {
            peer_connection,
            video_track,
        })
    }

    async fn start_streaming(&self, video_path: &str) -> Result<()> {
        let ffmpeg_args = [
            "-re",
            "-stream_loop",
            "-1",
            "-i",
            video_path,
            "-c:v",
            "libx264",
            "-profile:v",
            "baseline",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-f",
            "rtp",
            "-",
        ];

        let mut child = Command::new("ffmpeg")
            .args(&ffmpeg_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn ffmpeg process")?;

        let stdout = child
            .stdout
            .take()
            .context("Failed to get stdout from ffmpeg")?;
        let stdout = tokio::process::ChildStdout::from_std(stdout)?;
        let mut stdout_reader = tokio::io::BufReader::new(stdout);
        let video_track = Arc::clone(&self.video_track);

        self.peer_connection
            .add_track(Arc::clone(&self.video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await
            .context("Failed to add video track")?;

        tokio::spawn(async move {
            let mut buffer = [0u8; 1500];
            while let Ok(n) = stdout_reader.read(&mut buffer).await {
                if n == 0 {
                    break;
                }

                if let Err(e) = video_track
                    .write_sample(&webrtc::media::Sample {
                        data: buffer[..n].to_vec().into(),
                        duration: std::time::Duration::from_millis(33),
                        ..Default::default()
                    })
                    .await
                {
                    error!("Error writing sample: {}", e);
                }
            }
        });

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    env_logger::init();

    info!("Starting video streamer...");

    let streamer = VideoStreamer::new()
        .await
        .context("Failed to create VideoStreamer")?;
    let ws = tokio_tungstenite::connect_async("ws://127.0.0.1:3030/signaling")
        .await
        .context("WebSocket connection failed")?
        .0;
    let (ws_tx, mut ws_rx) = ws.split();

    let ws_tx_clone = Arc::new(Mutex::new(ws_tx));
    let pc = Arc::clone(&streamer.peer_connection);
    let ws_tx_for_ice = Arc::clone(&ws_tx_clone);

    pc.on_ice_candidate(Box::new(move |c| {
        let ws_tx = Arc::clone(&ws_tx_for_ice);
        Box::pin(async move {
            if let Some(candidate) = c {
                let msg = serde_json::to_string(&SignalingMessage::Candidate {
                    candidate: candidate.to_json().unwrap(),
                })
                .unwrap();
                if let Err(e) = ws_tx.lock().await.send(WsMessage::Text(msg)).await {
                    error!("Failed to send ICE candidate: {}", e);
                }
            }
        })
    }));

    streamer
        .start_streaming("~/Desktop/IMG_0612.mp4")
        .await
        .context("Failed to start streaming")?;

    let offer = streamer
        .peer_connection
        .create_offer(None)
        .await
        .context("Failed to create offer")?;
    streamer
        .peer_connection
        .set_local_description(offer.clone())
        .await
        .context("Failed to set local description")?;

    let offer_msg = serde_json::to_string(&SignalingMessage::Offer { sdp: offer })?;
    ws_tx_clone
        .lock()
        .await
        .send(WsMessage::Text(offer_msg))
        .await
        .context("Failed to send offer message")?;

    while let Some(msg) = ws_rx.next().await {
        let msg = msg.context("Error receiving message")?;
        if let WsMessage::Text(text) = msg {
            if let Ok(signal) = serde_json::from_str::<SignalingMessage>(&text) {
                match signal {
                    SignalingMessage::Answer { sdp } => {
                        streamer
                            .peer_connection
                            .set_remote_description(sdp)
                            .await
                            .context("Failed to set remote description")?;
                    }
                    SignalingMessage::Candidate { candidate } => {
                        streamer
                            .peer_connection
                            .add_ice_candidate(candidate)
                            .await
                            .context("Failed to add ICE candidate")?;
                    }
                    _ => {}
                }
            } else {
                warn!("Received non-JSON message: {}", text);
            }
        }
    }

    Ok(())
}
