use async_trait::async_trait;
use azure_storage_blobs::prelude::*;
use futures::StreamExt;
use std::path::Path;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use url::Url;

use crate::uploader::Uploader;

pub struct AzureUploader {
    container: ContainerClient,
    prefix: String,
}

impl AzureUploader {
    pub fn create(url: &Url, prefix: &str) -> Self {
        Self {
            container: ContainerClient::from_sas_url(url).unwrap(),
            prefix: prefix.to_owned(),
        }
    }
}

#[async_trait]
impl Uploader for AzureUploader {
    async fn upload(&self, path: &Path) -> anyhow::Result<()> {
        let mut name = self.prefix.clone();
        name.push_str(path.file_name().unwrap().to_str().unwrap());
        let blob = self.container.blob_client(name);
        blob.put_append_blob().await?;
        let file = File::open(path).await?;
        let capacity = match path.extension() {
            Some(str) => match str.to_str() {
                Some("mp4") => 0x10000usize,
                _ => 0x1000usize,
            },
            _ => 0x1000usize,
        };
        let mut stream = ReaderStream::with_capacity(file, capacity);
        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            blob.append_block(bytes).await?;
        }
        Ok(())
    }
}
