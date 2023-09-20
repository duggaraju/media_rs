use crate::config::AppConfig;
use anyhow::anyhow;
use async_trait::async_trait;
use azure_core::{date, Url};
use azure_storage::{prelude::BlobSasPermissions, ConnectionString};
use azure_storage_blobs::prelude::*;
use futures::{Stream, StreamExt};
use storage::{StorageContainer, StorageError, StreamType};
use time::OffsetDateTime;

#[async_trait]
pub trait StorageServer {
    async fn get_video(
        &self,
        account: &str,
        video: &str,
    ) -> anyhow::Result<Box<dyn StorageContainer>>;
}

#[derive(Clone)]
pub struct AzureStorage {
    config: AppConfig,
}

impl AzureStorage {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub async fn check_video(&self, container: &str, file: &str) -> azure_core::Result<()> {
        let connection_string = ConnectionString::new(&self.config.storage)?;
        let blob_service = BlobServiceClient::new(
            connection_string.account_name.unwrap(),
            connection_string.storage_credentials()?,
        );

        let container_client = blob_service.container_client(container);
        container_client.get_properties().into_future().await?;
        let blob_client = container_client.blob_client(file);
        blob_client.get_properties().into_future().await?;
        Ok(())
    }

    pub async fn get_media_duration(&self, file: &str) -> anyhow::Result<f32> {
        let connection_string = ConnectionString::new(&self.config.storage)?;
        let blob_service = BlobServiceClient::new(
            connection_string.account_name.unwrap(),
            connection_string.storage_credentials()?,
        );

        let blob_client = blob_service.container_client(container).blob_client(file);

        let result = blob_client.get_properties().into_future().await?;
        if let Some(metadata) = result.blob.metadata {
            if let Some(dur) = metadata.get("duration") {
                let duration = dur.parse::<f32>()?;
                return Ok(duration);
            }
        }
        Err(anyhow!("Missing metadata!"))
    }

    pub async fn get_media_file(
        &self,
        container: &str,
        file: &str,
    ) -> azure_core::Result<impl Stream<Item = azure_core::Result<bytes::Bytes>>> {
        let connection_string = ConnectionString::new(&self.config.storage)?;
        let blob_service = BlobServiceClient::new(
            connection_string.account_name.unwrap(),
            connection_string.storage_credentials()?,
        );

        let container_client = blob_service.container_client(container);
        container_client.get_properties().into_future().await?;
        let blob_client = container_client.blob_client(file);
        blob_client.get_properties().into_future().await?;
        let result = blob_client
            .get()
            .chunk_size(64000_u64)
            .into_stream()
            .map(|r| r.map(|p| p.data))
            .then(|r| async move {
                match r {
                    Ok(d) => d.collect().await,
                    Err(e) => Err(e),
                }
            });
        Ok(result)
    }
}

#[async_trait]
impl StorageServer for AzureStorage {
    async fn get_video(
        &self,
        account: &str,
        video: &str,
    ) -> anyhow::Result<Box<dyn StorageContainer>> {
        let connection_string = ConnectionString::new(&self.config.storage)?;
        let blob_service = BlobServiceClient::new(
            connection_string.account_name.unwrap(),
            connection_string.storage_credentials()?,
        );

        Ok(Box::new(AzureStorageContainer::new(container)))
    }
}

struct AzureStorageContainer {
    container: ContainerClient,
}

impl AzureStorageContainer {
    pub fn new(container: ContainerClient) -> Self {
        Self { container }
    }

    pub fn get_sas_url(
        &self,
        container: &str,
        file: &str,
        read_only: bool,
    ) -> azure_core::Result<Url> {
        let blob_client = self.container.blob_client(file);
        let expiry = OffsetDateTime::now_utc() + date::duration_from_minutes(1);
        let permissions = BlobSasPermissions {
            read: true,
            write: !read_only,
            ..Default::default()
        };
        let sas = blob_client.shared_access_signature(permissions, expiry)?;
        let url = blob_client.generate_signed_blob_url(&sas)?;
        Ok(url)
    }
}

#[async_trait]
impl StorageContainer for AzureStorageContainer {
    async fn get_content(&self, path: &str) -> Result<StreamType, StorageError> {
        let blob_client = self.container.blob_client(path);
        blob_client.get_properties().into_future().await?;
        let result = blob_client
            .get()
            .chunk_size(64000_u64)
            .into_stream()
            .map(|r| r.map(|p| p.data))
            .then(|r| async move {
                match r {
                    Ok(d) => d.collect().await,
                    Err(e) => Err(e),
                }
            });
        Ok(result)
    }

    async fn get_metadata(&self, path: &str) -> Result<String, StorageError> {
        todo!()
    }

    async fn set_content(&self, path: &str, content: StreamType) -> Result<(), StorageError> {
        todo!()
    }

    async fn set_metadata(&self, path: &str, metadata: String) -> Result<(), StorageError> {
        todo!()
    }

    async fn exists(&self, path: &str) -> bool {
        let blob_client = self.container.blob_client(path);
        blob_client.get_properties().into_future().await.is_ok()
    }
}
