use config::{Config, ConfigError, Environment};

#[derive(Debug, Default, serde_derive::Deserialize, PartialEq, Eq, Clone)]
pub struct AppConfig {
    pub storage: String,
    pub pod_address: String,
    pub node_address: String,
    pub storage_port: i32,
    pub stream_while_encoding: bool,
    pub use_gpu: bool,
    pub encode_ahead: bool,
    pub cache_fragments: bool,
}

#[derive(Debug, Default, serde_derive::Deserialize, PartialEq, Eq, Clone)]
pub struct JobConfig {
    pub registry_name: String,
    pub image_name: String,
    pub gpu_image_name: String,
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(Environment::with_prefix("jit").separator("__"))
            .set_default("stream_while_encoding", true)?
            .set_default("pod_address", "127.0.0.1")?
            .set_default("node_address", "127.0.0.1")?
            .set_default("storage_port", 8080)?
            .set_default("use_gpu", false)?
            .set_default("encode_ahead", false)?
            .set_default("cache_fragments", true)?
            .build()?;
        config.try_deserialize()
    }
}

impl JobConfig {
    pub fn new() -> Self {
        JobConfig {
            registry_name: "livestream.azurecr.io/".to_string(),
            image_name: "ffmpeg".to_string(),
            gpu_image_name: "nvidia-ffmpeg".to_string(),
        }
    }
}
