use reqwest::blocking::{Client, Response};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GithubCommit {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GithubListCommitsResponseItem {
    sha: String,
    commit: GithubCommit,
}

pub(super) trait DataRepository {
    fn fetch(&self, version: String) -> Option<String>;
}

pub(super) struct McmetaRemoteRepository;

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
