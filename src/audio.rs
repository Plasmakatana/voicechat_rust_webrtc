use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use ringbuf::RingBuffer;
use std::sync::Arc;
use tokio::sync::Mutex;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

pub struct AudioManager {
    input_device: cpal::Device,
    output_device: cpal::Device,
    config: cpal::StreamConfig,
    input_stream: Arc<Mutex<Option<cpal::Stream>>>,
    output_stream: Arc<Mutex<Option<cpal::Stream>>>,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        
        let input_device = host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;
        
        let output_device = host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device available"))?;

        let config: cpal::StreamConfig = input_device.default_input_config()?.into();

        Ok(Self {
            input_device,
            output_device,
            config,
            input_stream: Arc::new(Mutex::new(None)),
            output_stream: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn create_track(&self) -> Result<TrackLocalStaticRTP> {
        // Create a new RTP track for audio
        let track = TrackLocalStaticRTP::new(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
                mime_type: "audio/opus".to_owned(),
                ..Default::default()
            },
            "audio".to_owned(),
            "webrtc-rs".to_owned(),
        );

        Ok(track)
    }

    pub async fn start_capturing(&self) -> Result<()> {
        let (mut producer, mut consumer) = RingBuffer::<f32>::new(1024).split();

        // Set up input stream
        let input_stream = self.input_device.build_input_stream(
            &self.config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                for &sample in data {
                    let _ = producer.push(sample);
                }
            },
            move |err| eprintln!("Error in input stream: {}", err),
        )?;

        // Set up output stream
        let output_stream = self.output_device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    *sample = consumer.pop().unwrap_or(0.0);
                }
            },
            move |err| eprintln!("Error in output stream: {}", err),
        )?;

        input_stream.play()?;
        output_stream.play()?;

        *self.input_stream.lock().await = Some(input_stream);
        *self.output_stream.lock().await = Some(output_stream);

        Ok(())
    }

    pub async fn stop_capturing(&self) -> Result<()> {
        if let Some(stream) = self.input_stream.lock().await.take() {
            drop(stream);
        }
        if let Some(stream) = self.output_stream.lock().await.take() {
            drop(stream);
        }
        Ok(())
    }
}