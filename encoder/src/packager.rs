use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::Stdio,
};

use log::info;
use tokio::process::Command;

use crate::preset::Preset;

pub enum StreamType {
    Audio,
    Video,
}

pub struct PackagerStream {
    pub input: PathBuf,
    pub output: PathBuf,
    pub manifest: PathBuf,
    pub stream_type: StreamType,
}

pub struct Packager {
    options: PackagerOptions,
}

pub struct PackagerFiles {
    pub streams: Vec<PackagerStream>,
    pub manifest: PackagerStream,
}
pub struct PackagerOptions {
    pub manifest_name: PathBuf,
    pub pipes: bool,
    pub command: String,
}

pub struct EncryptionOptions {
    pub key_id: String,
    pub key: String,
    pub hls_key_uri: Option<String>,
}

impl Display for StreamType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            StreamType::Audio => write!(fmt, "audio"),
            StreamType::Video => write!(fmt, "video"),
        }
    }
}

impl PackagerStream {
    pub fn create(stream_type: StreamType, input: &Path, dir: &Path, name: &str) -> Self {
        let mut output = dir.join(name);
        output.set_extension("mp4");
        let mut manifest = dir.join(name);
        manifest.set_extension("m3u8");
        PackagerStream {
            stream_type,
            input: input.to_path_buf(),
            output,
            manifest,
        }
    }
}

impl PackagerFiles {
    pub fn create(streams: Vec<PackagerStream>, manifest: PackagerStream) -> Self {
        Self { streams, manifest }
    }

    #[cfg(unix)]
    pub fn create_pipes(&self, output_pipe: bool) -> anyhow::Result<()> {
        info!("creating named pipes!");
        for stream in &self.streams {
            unix_named_pipe::create(stream.input.as_path(), None)?;
            if output_pipe {
                unix_named_pipe::create(stream.output.as_path(), None)?;
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    pub fn create_pipes(&self, _: bool) -> anyhow::Result<()> {
        Ok(())
    }
}

impl PackagerOptions {
    pub fn create(manifest_name: PathBuf) -> Self {
        PackagerOptions {
            manifest_name,
            pipes: !cfg!(windows),
            command: if cfg!(windows) {
                "packager-win-x64.exe"
            } else {
                "packager"
            }
            .to_owned(),
        }
    }

    pub fn get_files(
        &self,
        preset: &Preset,
        directory: &Path,
        inputs: &[PathBuf],
    ) -> PackagerFiles {
        let videos = preset.videos.len();

        let streams: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                PackagerStream::create(
                    if i < videos {
                        StreamType::Video
                    } else {
                        StreamType::Audio
                    },
                    input,
                    directory,
                    &format!(
                        "{}_{}",
                        if i < videos { "video" } else { "audio" },
                        if i < videos { i } else { videos - i },
                    ),
                )
            })
            .collect();

        // Top level manifests can't use pipe.
        let manifest = PackagerStream {
            stream_type: StreamType::Video,
            input: PathBuf::new(),
            output: directory.join(self.manifest_name.with_extension(".mpd")),
            manifest: directory.join(self.manifest_name.with_extension(".m3u8")),
        };
        PackagerFiles::create(streams, manifest)
    }
}

impl Packager {
    pub fn new(options: PackagerOptions) -> Self {
        Self { options }
    }

    pub async fn run(
        self,
        files: &PackagerFiles,
        options: Option<EncryptionOptions>,
    ) -> anyhow::Result<()> {
        let mut command = Command::new(&self.options.command);
        command
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stdin(Stdio::null());

        for stream in &files.streams {
            command.arg(format!(
                "stream={},in={},format=mp4,out={},playlist_name={}",
                stream.stream_type,
                stream.input.display(),
                stream.output.display(),
                stream.manifest.display()
            ));
        }
        if let Some(options) = &options {
            command
                .arg("--enable_raw_key_encryption")
                .arg("--protection_scheme")
                .arg("cbcs")
                .arg("--keys")
                .arg(format!(
                    "label=cenc:key_id={}:key={}",
                    options.key_id, options.key
                ))
                .arg("--clear_lead")
                .arg("0");

            if let Some(license_uri) = &options.hls_key_uri {
                command.args(["--hls_key_uri", license_uri]);
            }
        }

        command
            .arg("--vmodule=*=1")
            .arg("--mpd_output")
            .arg(&files.manifest.output)
            .arg("--hls_master_playlist_output")
            .arg(&files.manifest.manifest);

        info!("running packager {:?}", command);
        command.spawn()?.wait().await?;
        info!("packager finished!");
        Ok(())
    }
}
