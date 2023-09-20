use log::info;

use crate::location::Location;

pub struct Video {
    pub width: i32,
    pub height: i32,
    pub bitrate: String,
    pub size: String,
}

pub struct Audio {
    pub bitrate: String,
    pub channels: String,
}

pub struct Preset {
    pub video_codec: String,
    pub audio_codec: String,
    pub videos: Vec<Video>,
    pub audios: Vec<Audio>,
}

impl Video {
    fn create(width: i32, height: i32, bitrate: i32) -> Self {
        Video {
            width,
            height,
            bitrate: bitrate.to_string(),
            size: format!("{width}x{height}"),
        }
    }
}

impl Audio {
    fn create(bitrate: i32) -> Self {
        Audio {
            bitrate: bitrate.to_string(),
            channels: 2.to_string(),
        }
    }
}

impl Preset {
    pub fn h264_720p() -> Self {
        Preset {
            videos: vec![
                Video::create(1200, 720, 1500000),
                Video::create(850, 480, 750000),
                Video::create(850, 480, 750000),
            ],
            audios: vec![Audio::create(64000)],
            video_codec: "libx264".to_owned(),
            audio_codec: "aac".to_owned(),
        }
    }

    pub fn adaptive_preset(location: &Location) -> Self {
        info!("running ffprobe on {}", location.to_str());
        let _result = ffprobe::ffprobe(location.to_str()).unwrap();
        Self::h264_720p()
    }
}
