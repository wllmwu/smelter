use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use std::{collections::BTreeMap, fs};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BrigadierJsonNodeType {
    Argument,
    Literal,
    Root,
}

#[derive(Debug, Deserialize)]
pub struct BrigadierJsonNode {
    #[serde(rename = "type")]
    pub node_type: BrigadierJsonNodeType,
    pub children: Option<BTreeMap<String, BrigadierJsonNode>>,
    pub executable: Option<bool>,
    pub parser: Option<String>,
    pub properties: Option<BTreeMap<String, serde_json::Value>>,
    pub redirect: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GithubCommit {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GithubListCommitsResponseItem {
    sha: String,
    commit: GithubCommit,
}

trait DataRepository {
    fn fetch(&self, version: String) -> Option<String>;
}

struct McmetaRemoteRepository;

impl DataRepository for McmetaRemoteRepository {
    fn fetch(&self, version: String) -> Option<String> {
        let client: Client = Client::builder()
            .user_agent("wllmwu/smelter")
            .build()
            .expect("");
        let response: Response = client
            .get("https://api.github.com/repos/misode/mcmeta/commits?sha=summary")
            .send()
            .expect("");
        let results: Vec<GithubListCommitsResponseItem> = response.json().expect("");
        let commit_info: Vec<(&String, &String)> = results
            .iter()
            .map(|item| (&item.commit.message, &item.sha))
            .collect();
        for (message, sha) in commit_info {
            if message.contains(&version) {
                let response: Response = reqwest::blocking::get(format!(
                    "https://raw.githubusercontent.com/misode/mcmeta/{}/commands/data.json",
                    sha
                ))
                .expect("");
                return Some(response.text().expect(""));
            }
        }
        None
    }
}

trait DataCache {
    fn get(&self, version: String) -> String;
}

struct FileSystemDataCache {
    repository: Box<dyn DataRepository>,
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

pub struct DataProvider {
    cache: Box<dyn DataCache>,
}

impl DataProvider {
    pub fn new() -> Self {
        Self {
            cache: Box::new(FileSystemDataCache {
                repository: Box::new(McmetaRemoteRepository),
            }),
        }
    }

    pub fn get_command_data(&self, version: String) -> BrigadierJsonNode {
        let data: String = self.cache.get(version);
        serde_json::from_str(&data).unwrap()
    }
}
