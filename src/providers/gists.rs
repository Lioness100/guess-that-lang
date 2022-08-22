use std::{collections::BTreeMap, result};

use rand::{seq::SliceRandom, thread_rng, Rng};
use serde::Deserialize;
use ureq::Agent;

use crate::{
    game::LANGUAGES,
    providers::{AuthenticationExt, CodeData, GithubProvider, GITHUB_BASE_URL},
    Result,
};

#[derive(Deserialize)]
pub struct Gist {
    pub files: BTreeMap<String, GistFile>,
}

#[derive(Deserialize)]
pub struct GistFile {
    pub language: Option<String>,
    pub raw_url: String,
}

pub struct GistData {
    pub url: String,
    pub language: String,
}

impl TryFrom<Gist> for GistData {
    type Error = ();

    /// Create a new [`GistData`] struct from a [`Gist`]. This will return [`None`]
    /// if none of the gist files use one of the supported languages.
    fn try_from(gist: Gist) -> result::Result<Self, Self::Error> {
        let file = gist
            .files
            .into_values()
            .find(|file| {
                file.language
                    .as_ref()
                    .map_or(false, |language| LANGUAGES.contains(&language.as_str()))
            })
            .ok_or(())?;

        Ok(Self {
            url: file.raw_url.to_string(),
            language: file.language.unwrap(),
        })
    }
}

pub struct GistProvider {
    agent: Agent,
    token: Option<String>,
    cache: Vec<GistData>,
}

impl GistProvider {
    /// Get a vec of random valid gists on Github. This is used with the assumption
    /// that at least one valid gist will be found.
    pub fn get_gists(&self) -> Result<Vec<GistData>> {
        let mut gists: Vec<_> = self
            .agent
            .get(&format!("{GITHUB_BASE_URL}/gists/public"))
            .query("page", &thread_rng().gen_range(0..=100).to_string())
            .with_authentication(self.token.as_ref())
            .call()?
            .into_json::<Vec<Gist>>()?
            .into_iter()
            .filter_map(|gist| GistData::try_from(gist).ok())
            .collect();

        gists.shuffle(&mut thread_rng());

        Ok(gists)
    }

    /// Get the language options for a round. This will choose 3 random unique
    /// languages, push them to a vec along with the correct language, and
    /// shuffle the vec.
    #[must_use]
    pub fn get_options(correct_language: String) -> Vec<String> {
        let mut options = Vec::<String>::with_capacity(4);
        options.push(correct_language);

        let mut thread_rng = thread_rng();
        while options.len() < 4 {
            let random_language = (*LANGUAGES.choose(&mut thread_rng).unwrap()).to_string();
            if !options.contains(&random_language) {
                options.push(random_language);
            }
        }

        options.shuffle(&mut thread_rng);
        options
    }
}

impl GithubProvider for GistProvider {
    fn new() -> Result<Self> {
        let agent = Self::get_agent();
        let token = Self::apply_token(&agent)?;

        Ok(Self {
            agent,
            token,
            cache: Vec::with_capacity(0),
        })
    }

    fn get_code(&mut self) -> Result<CodeData> {
        if self.cache.is_empty() {
            self.cache = self.get_gists()?;
        };

        let gist = self.cache.pop().unwrap();

        Ok(CodeData {
            code: self
                .agent
                .get(&gist.url)
                .with_authentication(self.token.as_ref())
                .call()?
                .into_string()?,
            language: gist.language.clone(),
        })
    }
}
