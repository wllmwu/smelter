use std::{error, fmt, fs, io};

use repository::{DataRepository, McmetaRemoteRepository, RepositoryError};

mod repository;

/* * * * Public interface * * * */

#[derive(Debug)]
pub enum CacheError {
    VersionNotFound(RepositoryError),
    WriteFailure(io::Error),
}

pub(super) trait DataCache {
    fn get(&self, version: String) -> Result<String, CacheError>;
}

pub(super) struct FileSystemDataCache {
    repository: Box<dyn DataRepository>,
}

/* * * * Private implementation * * * */

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VersionNotFound(e) => {
                write!(f, "version not found: {e}")
            }
            Self::WriteFailure(e) => {
                write!(f, "copying data to cache failed: {e}")
            }
        }
    }
}

impl error::Error for CacheError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::VersionNotFound(e) => Some(e),
            Self::WriteFailure(e) => Some(e),
        }
    }
}

impl From<RepositoryError> for CacheError {
    fn from(value: RepositoryError) -> Self {
        Self::VersionNotFound(value)
    }
}

impl From<io::Error> for CacheError {
    fn from(value: io::Error) -> Self {
        Self::WriteFailure(value)
    }
}

impl FileSystemDataCache {
    pub(super) fn new() -> Self {
        Self {
            repository: Box::new(McmetaRemoteRepository),
        }
    }
}

impl DataCache for FileSystemDataCache {
    fn get(&self, version: String) -> Result<String, CacheError> {
        let path: String = format!("data/{version}.json");
        match fs::read_to_string(path.clone()) {
            Ok(data) => Ok(data),
            Err(error) => match error.kind() {
                io::ErrorKind::NotFound => {
                    let data: String = self.repository.fetch(version)?;
                    fs::write(path, data.clone())?;
                    Ok(data)
                }
                _ => Err(CacheError::WriteFailure(error)),
            },
        }
    }
}
