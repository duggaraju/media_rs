use std::{ffi::OsString, path::PathBuf};
use url::Url;

pub enum Location {
    Path(PathBuf),
    Url(Url),
}

impl Location {
    pub fn parse(str: OsString) -> Self {
        if let Ok(url) = Url::parse(str.to_str().unwrap()) {
            Self::Url(url)
        } else {
            Self::Path(PathBuf::from(str))
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Self::Url(url) => url.as_str(),
            Self::Path(path) => path.to_str().unwrap(),
        }
    }
}
