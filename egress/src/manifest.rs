use std::time::Duration;

use hls_m3u8::{
    tags::VariantStream,
    types::{StreamData, UFloat},
    MasterPlaylist, MediaPlaylist, MediaSegment,
};
use serde::{Deserialize, Serialize};
use storage::StorageContainer;

pub struct Variant {
    pub width: usize,
    pub height: usize,
    pub bandwidth: u64,
}

const FRAME_RATE: f32 = 30.0;
pub const SEGMENT_DURATION: u32 = 5;

const CODECS: [&str; 2] = ["avc1.42e00a", "mp4a.40.2"];

pub const VARIANTS: [Variant; 3] = [
    Variant {
        width: 1920,
        height: 1080,
        bandwidth: 2000000,
    },
    Variant {
        width: 1200,
        height: 720,
        bandwidth: 1000000,
    },
    Variant {
        width: 848,
        height: 480,
        bandwidth: 600000,
    },
];

pub struct ManifestServer {
    storage: Box<dyn StorageContainer>,
}

#[derive(Serialize, Deserialize)]
struct Format {
    pub duration: f64,
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    pub format: Format,
}

impl ManifestServer {
    pub fn new(storage: impl StorageContainer) -> Self {
        ManifestServer {
            storage: Box::new(storage),
        }
    }

    pub async fn get_variant_playlist(&self, video: String) -> anyhow::Result<String> {
        let file_name: String = video[..video.len() - 5].into();
        if !self.storage.exists(&file_name).await {
            return Err(anyhow!("failed to find the video {}", video));
        }
        let variants: Vec<_> = VARIANTS
            .into_iter()
            .enumerate()
            .map(|(i, v)| VariantStream::ExtXStreamInf {
                uri: format!("{}/level{}.m3u8", file_name, i).into(),
                audio: None,
                frame_rate: Some(UFloat::new(FRAME_RATE)),
                subtitles: None,
                closed_captions: None,
                stream_data: StreamData::builder()
                    .bandwidth(v.bandwidth)
                    .codecs(CODECS)
                    .resolution((v.width, v.height))
                    .build()
                    .unwrap(),
            })
            .collect();
        let playlist = MasterPlaylist::builder()
            .variant_streams(variants)
            .build()
            .unwrap();
        Ok(playlist.to_string())
    }

    pub async fn get_media_playlist(&self, video: String, level: u32) -> anyhow::Result<String> {
        let duration = self.get_media_duration("", &container, &video).await?;
        let num_segments = duration as u32 / SEGMENT_DURATION;
        let segment_duration = Duration::from_secs(SEGMENT_DURATION as u64);
        let segments: Vec<_> = (0..num_segments)
            .map(|i| {
                MediaSegment::builder()
                    .duration(segment_duration)
                    .uri(format!("level{}/segment{}.ts", level, i))
                    .build()
                    .unwrap()
            })
            .collect();
        let playlist = MediaPlaylist::builder()
            .target_duration(Duration::from_secs(SEGMENT_DURATION as u64))
            .segments(segments)
            .has_end_list(true)
            .build()
            .unwrap();
        Ok(playlist.to_string())
    }

    async fn get_media_duration(
        &self,
        account: &str,
        video: &str,
        path: &str,
    ) -> anyhow::Result<f64> {
        let metadata = self.storage.get_metadata(path).await?;
        let value = serde_json::from_str::<Metadata>(&metadata)?;
        Ok(value.format.duration)
    }
}
