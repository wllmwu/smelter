use std::{error, fmt};

use reqwest::blocking::{Client, Response};
use serde::Deserialize;

/* * * * Public interface * * * */

#[derive(Debug)]
pub enum RepositoryError {
    DeserializationError(reqwest::Error),
    RequestError(reqwest::Error),
    VersionNotFound(String),
}

pub(super) trait DataRepository {
    fn fetch(&self, version: String) -> Result<String, RepositoryError>;
}

pub(super) struct McmetaRemoteRepository;

/* * * * Private implementation * * * */

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeserializationError(e) => {
                write!(f, "response deserialization failed: {e}")
            }
            Self::RequestError(e) => {
                write!(f, "network request failed: {e}")
            }
            Self::VersionNotFound(s) => {
                write!(f, "version '{s}' not found in repository")
            }
        }
    }
}

impl error::Error for RepositoryError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::DeserializationError(e) => Some(e),
            Self::RequestError(e) => Some(e),
            Self::VersionNotFound(_) => None,
        }
    }
}

impl From<reqwest::Error> for RepositoryError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_decode() {
            Self::DeserializationError(value)
        } else {
            Self::RequestError(value)
        }
    }
}

#[derive(Deserialize)]
struct GithubCommit {
    message: String,
}

#[derive(Deserialize)]
struct GithubListCommitsResponseItem {
    sha: String,
    commit: GithubCommit,
}

impl DataRepository for McmetaRemoteRepository {
    fn fetch(&self, version: String) -> Result<String, RepositoryError> {
        let client: Client = Client::builder()
            .user_agent("wllmwu/smelter")
            .build()
            .expect("Request client should build properly");
        let response: Response = client
            .get("https://api.github.com/repos/misode/mcmeta/commits?sha=summary")
            .send()?;
        let results: Vec<GithubListCommitsResponseItem> = response.json()?;
        let commit_info: Vec<(&String, &String)> = results
            .iter()
            .map(|item| (&item.commit.message, &item.sha))
            .collect();
        for (message, sha) in commit_info {
            if message.contains(&version) {
                let response: Response = reqwest::blocking::get(format!(
                    "https://raw.githubusercontent.com/misode/mcmeta/{sha}/commands/data.json"
                ))?;
                let data: String = response.text()?;
                return Ok(data);
            }
        }
        Err(RepositoryError::VersionNotFound(version))
    }
}
