use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Button, Box, Orientation, Entry};
use std::sync::Arc;
use tokio::sync::mpsc;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};
use webrtc::api::API;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use anyhow::Result;

mod audio;
mod signaling;

use audio::AudioManager;
use signaling::SignalingClient;

const APP_ID: &str = "com.example.p2p-voice-chat";

#[derive(Clone)]
struct VoiceChatApp {
    peer_connection: Arc<RTCPeerConnection>,
    audio_manager: Arc<AudioManager>,
    signaling_client: Arc<SignalingClient>,
}

impl VoiceChatApp {
    async fn new() -> Result<Self> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()?;

        let mut registry = register_default_interceptors();
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        let config = RTCConfiguration::default();
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        let audio_manager = Arc::new(AudioManager::new()?);
        let signaling_client = Arc::new(SignalingClient::new());

        Ok(Self {
            peer_connection,
            audio_manager,
            signaling_client,
        })
    }

    async fn start_call(&self, remote_id: String) -> Result<()> {
        let audio_track = self.audio_manager.create_track().await?;
        self.peer_connection.add_track(Arc::new(audio_track)).await?;

        // Create offer
        let offer = self.peer_connection.create_offer(None).await?;
        self.peer_connection.set_local_description(offer.clone()).await?;

        // Send offer through signaling server
        self.signaling_client.send_offer(remote_id, offer).await?;

        // Start capturing audio
        self.audio_manager.start_capturing().await?;

        Ok(())
    }

    async fn handle_answer(&self, answer: webrtc::peer_connection::sdp::SessionDescription) -> Result<()> {
        self.peer_connection.set_remote_description(answer).await?;
        Ok(())
    }

    async fn end_call(&self) -> Result<()> {
        self.audio_manager.stop_capturing().await?;
        self.peer_connection.close().await?;
        Ok(())
    }
}

fn build_ui(app: &Application) {
    let app_instance = match tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(VoiceChatApp::new()) {
            Ok(instance) => instance,
            Err(e) => {
                eprintln!("Failed to create VoiceChatApp: {}", e);
                return;
            }
        };

    let window = ApplicationWindow::builder()
        .application(app)
        .title("P2P Voice Chat")
        .default_width(300)
        .default_height(200)
        .build();

    let container = Box::new(Orientation::Vertical, 6);
    
    let remote_id_entry = Entry::new();
    remote_id_entry.set_placeholder_text(Some("Enter remote ID"));
    
    let start_call_button = Button::with_label("Start Call");
    let end_call_button = Button::with_label("End Call");
    
    let app_clone = app_instance.clone();
    start_call_button.connect_clicked(move |_| {
        let remote_id = remote_id_entry.text().to_string();
        let app_clone = app_clone.clone();
        
        tokio::spawn(async move {
            if let Err(e) = app_clone.start_call(remote_id).await {
                eprintln!("Failed to start call: {}", e);
            }
        });
    });

    let app_clone = app_instance.clone();
    end_call_button.connect_clicked(move |_| {
        let app_clone = app_clone.clone();
        
        tokio::spawn(async move {
            if let Err(e) = app_clone.end_call().await {
                eprintln!("Failed to end call: {}", e);
            }
        });
    });
    
    container.pack_start(&remote_id_entry, false, false, 0);
    container.pack_start(&start_call_button, true, true, 0);
    container.pack_start(&end_call_button, true, true, 0);

    window.add(&container);
    window.show_all();
}

#[tokio::main]
async fn main() -> Result<()> {
    let application = Application::builder()
        .application_id(APP_ID)
        .build();

    application.connect_activate(build_ui);
    application.run();

    Ok(())
}
