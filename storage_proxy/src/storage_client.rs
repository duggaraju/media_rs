use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Body, Client};
use storage::{StorageContainer, StorageError, StreamType};

pub struct StorageConfig {
    pub storage_port: u32,
    pub node_address: String,
}

pub struct StorageClient {
    config: StorageConfig,
    client: reqwest::Client,
    account: String,
    video: String,
}

impl StorageClient {
    pub fn new(config: StorageConfig, account: &str, video: &str) -> Self {
        StorageClient {
            config,
            client: Client::new(),
            account: account.to_owned(),
            video: video.to_owned(),
        }
    }

    pub fn from_reqwest_error(error: reqwest::Error) -> StorageError {
        StorageError::HttpError(error.to_string())
    }

    fn get_url(&self, path: &str, metadata: bool) -> String {
        format!(
            "http://{}:{}/{}/{}/{}{}",
            self.config.node_address, self.config.storage_port, self.account, self.video, path, if metadata { "/metadata" } else { ""}
        )
    }
}

#[async_trait]
impl StorageContainer for StorageClient {
    async fn get_metadata(&self, path: &str) -> Result<String, StorageError> {
        let uri = self.get_url(path, true);
        let response = self
            .client
            .get(uri)
            .send()
            .await
            .map_err(Self::from_reqwest_error)?;
        let metadata = response.text().await.map_err(Self::from_reqwest_error)?;
        Ok(metadata)
    }

    async fn set_metadata(&self, path: &str, metadata: String) -> Result<(), StorageError> {
        let uri = self.get_url(path, true);
        self
            .client
            .post(uri)
            .body(metadata)
            .send()
            .await
            .map_err(Self::from_reqwest_error)?;
        Ok(())
    }

    async fn get_content(&self, path: &str) -> Result<StreamType, StorageError> {
        let uri = self.get_url(path, false);

        let res = self
            .client
            .get(uri)
            .send()
            .await
            .map_err(Self::from_reqwest_error)?;
        let stream = res
            .bytes_stream()
            .map(|f| f.map_err(Self::from_reqwest_error));
        Ok(Box::pin(stream))
    }

    async fn set_content(&self, path: &str, content: StreamType) -> Result<(), StorageError> {
        let uri = self.get_url(path, false);
        let pinned_content = Box::pin(content);
        let body = Body::wrap_stream(pinned_content);
        self
            .client
            .post(uri)
            .body(body)
            .send()
            .await
            .map_err(Self::from_reqwest_error)?;
        Ok(())
    }

    async fn exists(&self, path:&str) -> bool {
        let uri = self.get_url(path, false);
        self
            .client
            .head(uri)
            .send()
            .await
            .is_ok()
    }
}
