use crate::{packager::PackagerStream, preset::Preset};

use std::process::Stdio;

use ffmpeg_cli::{FfmpegBuilder, File, Parameter};
use futures::{future::ready, StreamExt};
use log::info;

pub struct Encoder {
    preset: Preset,
}

impl Encoder {
    pub fn create(preset: Preset) -> Self {
        Encoder { preset }
    }

    pub async fn run(self, input: &str, outputs: &[PackagerStream]) -> anyhow::Result<()> {
        let mut builder = FfmpegBuilder::new()
            .stderr(Stdio::inherit())
            .option(Parameter::Single("y"))
            .input(File::new(input));

        for (i, video) in self.preset.videos.iter().enumerate() {
            builder = builder.output(
                File::new(outputs[i].input.to_str().unwrap())
                    .option(Parameter::Single("an"))
                    .option(Parameter::KeyValue("c:v", &self.preset.video_codec))
                    .option(Parameter::KeyValue("s", &video.size))
                    .option(Parameter::KeyValue("b:v", &video.bitrate))
                    .option(Parameter::KeyValue("r", "30"))
                    .option(Parameter::KeyValue("g", "60"))
                    .option(Parameter::KeyValue(
                        "movflags",
                        "cmaf+delay_moov+skip_trailer+skip_sidx+frag_keyframe",
                    )),
            );
        }

        let videos = self.preset.videos.len();
        for (i, audio) in self.preset.audios.iter().enumerate() {
            builder = builder.output(
                File::new(outputs[i + videos].input.to_str().unwrap())
                    .option(Parameter::Single("vn"))
                    .option(Parameter::KeyValue("c:a", &self.preset.audio_codec))
                    .option(Parameter::KeyValue("b:a", &audio.bitrate))
                    .option(Parameter::KeyValue(
                        "movflags",
                        "cmaf+delay_moov+skip_trailer+skip_sidx",
                    ))
                    .option(Parameter::KeyValue("frag_duration", "2")),
            );
        }

        let mut ffmpeg = builder.run().await.unwrap();

        ffmpeg
            .progress
            .for_each(|_x| {
                //dbg!("{}", x.unwrap());
                ready(())
            })
            .await;

        let output = ffmpeg.process.wait()?;

        info!("ffmpeg finished: {} ", output);
        Ok(())
    }
}
