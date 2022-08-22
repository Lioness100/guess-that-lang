// Inspired by https://github.com/ModProg/language-guesser.

use std::collections::HashMap;

use rand::{seq::SliceRandom, thread_rng, Rng};
use serde::Deserialize;
use ureq::Agent;

use crate::{
    game::LANGUAGES,
    providers::{AuthenticationExt, CodeData, GithubProvider, GITHUB_BASE_URL},
    Result,
};

#[derive(Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
}

#[derive(Deserialize)]
pub struct Repository {
    pub full_name: String,
}

#[derive(Deserialize)]
pub struct RepositoryFilePreview {
    pub url: String,
}

#[derive(Deserialize)]
pub struct RepositoryFile {
    pub download_url: String,
}

pub struct RepositoryProvider<'a> {
    agent: Agent,
    token: Option<String>,
    cache: HashMap<&'a str, Vec<String>>,
}

impl RepositoryProvider<'_> {
    /// Get a vec of random valid gists on Github. This is used with the assumption
    /// that at least one valid gist will be found.
    pub fn get_repos(&self, language: &str) -> Result<Vec<String>> {
        let mut repos: Vec<_> = self
            .agent
            .get(&format!("{GITHUB_BASE_URL}/search/repositories"))
            .query("page", &thread_rng().gen_range(0..35).to_string())
            .query("q", &format!("language:{language} stars:>20 sort:updated"))
            .with_authentication(self.token.as_ref())
            .call()?
            .into_json::<Page<Repository>>()?
            .items
            .into_iter()
            .map(|repo| repo.full_name)
            .collect();

        repos.shuffle(&mut thread_rng());

        Ok(repos)
    }

    pub fn get_file(&self, language: &str, name: &str) -> Result<RepositoryFile> {
        let files = self
            .agent
            .get(&format!("{GITHUB_BASE_URL}/search/code"))
            .query("q", &format!("language:{language} repo:{name}"))
            .with_authentication(self.token.as_ref())
            .call()?
            .into_json::<Page<RepositoryFilePreview>>()?
            .items;

        let preview = files.choose(&mut thread_rng()).ok_or("")?;

        Ok(self
            .agent
            .get(&preview.url)
            .with_authentication(self.token.as_ref())
            .call()?
            .into_json::<_>()?)
    }
}

impl GithubProvider for RepositoryProvider<'_> {
    fn new() -> Result<Self> {
        let agent = Self::get_agent();
        let token = Self::apply_token(&agent)?;

        Ok(Self {
            agent,
            token,
            cache: HashMap::new(),
        })
    }

    fn get_code(&mut self) -> Result<CodeData> {
        let language = LANGUAGES.choose(&mut thread_rng()).unwrap();
        let cache = self.cache.get(language);

        if cache.map_or(true, Vec::is_empty) {
            self.cache.insert(language, self.get_repos(language)?);
        }

        let cache = self.cache.entry(language).or_default();
        let repo = (*cache).pop().unwrap();
        let file = self.get_file(language, &repo)?;

        Ok(CodeData {
            code: self
                .agent
                .get(&file.download_url)
                .with_authentication(self.token.as_ref())
                .call()?
                .into_string()?,
            language: (*language).to_string(),
        })
    }
}
