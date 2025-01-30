use repository::{DataRepository, McmetaRemoteRepository};
use std::fs;

mod repository;

pub(super) trait DataCache {
    fn get(&self, version: String) -> String;
}

pub(super) struct FileSystemDataCache {
    repository: Box<dyn DataRepository>,
}

impl FileSystemDataCache {
    pub(super) fn new() -> Self {
        Self {
            repository: Box::new(McmetaRemoteRepository),
        }
    }
}

impl DataCache for FileSystemDataCache {
    fn get(&self, version: String) -> String {
        let path: String = format!("data/{}.json", version);
        match fs::read_to_string(path.clone()) {
            Ok(s) => s,
            Err(_) => match self.repository.fetch(version) {
                Some(s) => match fs::write(path, s.clone()) {
                    Ok(_) => s,
                    Err(_) => panic!("Failed to write command data"),
                },
                None => panic!("Failed to get command data"),
            },
        }
    }
}
