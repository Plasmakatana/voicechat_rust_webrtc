use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::connect_async;
use url::Url;
use webrtc::peer_connection::sdp::SessionDescription;

#[derive(Serialize, Deserialize)]
enum SignalingMessage {
    Offer {
        offer: SessionDescription,
        target_id: String,
    },
    Answer {
        answer: SessionDescription,
        target_id: String,
    },
}

pub struct SignalingClient {
    ws_url: Url,
}

impl SignalingClient {
    pub fn new() -> Self {
        Self {
            ws_url: Url::parse("ws://localhost:8080").unwrap(),
        }
    }

    pub async fn send_offer(&self, remote_id: String, offer: SessionDescription) -> Result<()> {
        let (mut ws_stream, _) = connect_async(&self.ws_url).await?;
        
        let message = SignalingMessage::Offer {
            offer,
            target_id: remote_id,
        };
        
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&message)?,
        )).await?;
        
        Ok(())
    }

    pub async fn listen_for_answers(&self) -> Result<impl Stream<Item = Result<SessionDescription>>> {
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        
        Ok(ws_stream.filter_map(|msg| async move {
            match msg {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    if let Ok(SignalingMessage::Answer { answer, .. }) = serde_json::from_str(&text) {
                        Some(Ok(answer))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }))
    }
}