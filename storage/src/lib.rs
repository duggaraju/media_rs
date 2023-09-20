
use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("The file or directory is not found!")]
    NotFound,

    #[error("Authentication error!")]
    AuthenticationError,

    #[error("HTTP error")]
    HttpError(String),

    #[error("Other error")]
    Other(#[from]std::io::Error)
}

pub type StreamType = Pin<Box<dyn Stream<Item = std::result::Result<Bytes, StorageError>> + Send + Sync>>;

#[async_trait]
pub trait StorageContainer {
    async fn get_content(&self, path: &str) -> Result<StreamType, StorageError>;
    async fn get_metadata(&self, path: &str) -> Result<String, StorageError>;
    async fn set_content(
        &self,
        path: &str,
        content: StreamType,
    ) -> Result<(), StorageError>;
    async fn set_metadata(&self, path: &str, metadata: String) -> Result<(), StorageError>;
    async fn exists(&self, path:&str) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
