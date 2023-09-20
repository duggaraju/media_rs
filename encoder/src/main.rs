mod azure_storage;
mod encoder;
mod location;
mod packager;
mod preset;
mod uploader;

use azure_storage::AzureUploader;
use encoder::Encoder;
use futures::future::{join, join3};
use location::Location;
use log::info;
use packager::{Packager, PackagerOptions};
use preset::Preset;
use tempfile::TempDir;
use uploader::Uploader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut args = std::env::args_os();
    args.next();
    let input = Location::parse(args.next().unwrap());
    let output = Location::parse(args.next().unwrap());

    let preset = Preset::adaptive_preset(&input);
    let temp_dir = TempDir::new()?;
    info!("Temp directory is {}", temp_dir.path().to_str().unwrap());
    let mut output_dir = temp_dir.path();
    let path_prefix = "rust/";
    let uploader = match &output {
        Location::Url(url) => Some(AzureUploader::create(url, path_prefix)),
        Location::Path(path) => {
            output_dir = path.as_path();
            None
        }
    };
    let output_pipes = !cfg!(windows);

    let videos = preset.videos.len();
    let audios = preset.audios.len();
    let total = videos + audios;

    let inputs: Vec<_> = (0..total)
        .map(|i| temp_dir.path().join(format!("_input_{i}.mp4")))
        .collect();

    let manifest_name = output_dir.join("manifest");
    let options = PackagerOptions::create(manifest_name);

    let files = options.get_files(&preset, output_dir, &inputs);
    files.create_pipes(output_pipes)?;

    let packager = Packager::new(options).run(&files, None);
    let encoder = Encoder::create(preset).run(input.to_str(), &files.streams);

    if output_pipes {
        if let Some(uploader) = &uploader {
            let uploads = uploader.upload_media_file(&files);
            let (u, p, e) = join3(uploads, packager, encoder).await;
            u?;
            p?;
            e?;
        }
    } else {
        let (p, e) = join(packager, encoder).await;
        p?;
        e?;
    }

    info!("Uploading remaining files ...");
    if let Some(uploader) = uploader {
        uploader.upload_manifest_files(&files).await?;
        if !output_pipes {
            uploader.upload_media_file(&files).await?;
        }
    }

    Ok(())
}
