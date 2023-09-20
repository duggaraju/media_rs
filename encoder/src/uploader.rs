use std::path::Path;

use async_trait::async_trait;
use futures::future::{join_all, select_all};
use log::info;

use crate::packager::PackagerFiles;

#[async_trait]
pub trait Uploader {
    async fn upload(&self, path: &Path) -> anyhow::Result<()>;

    async fn upload_media_file(&self, files: &PackagerFiles) -> anyhow::Result<()> {
        let mut uploads: Vec<_> = files
            .streams
            .iter()
            .map(|s| self.upload(&s.output))
            .map(Box::pin)
            .collect();

        while !uploads.is_empty() {
            let (result, i, remaining) = select_all(uploads).await;
            info!(
                "Upload finished result {} = {:?}",
                &files.streams[i].output.to_str().unwrap(),
                result
            );
            result?;
            uploads = remaining;
        }
        info!("upload media files!");
        Ok(())
    }

    async fn upload_manifest_files(&self, files: &PackagerFiles) -> anyhow::Result<()> {
        let mut uploads: Vec<_> = files
            .streams
            .iter()
            .map(|s| self.upload(&s.manifest))
            .collect();

        // include top-level manifests.
        uploads.push(self.upload(&files.manifest.output));
        uploads.push(self.upload(&files.manifest.manifest));

        join_all(uploads).await;
        info!("upload manifest files finished!");
        Ok(())
    }
}
