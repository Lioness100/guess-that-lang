use std::{collections::BTreeMap, path::Path};

use anyhow::{bail, Context};
use rand::{seq::SliceRandom, thread_rng, Rng};
use regex::RegexBuilder;
use serde::Deserialize;
use syntect::parsing::SyntaxSet;
use ureq::{Agent, AgentBuilder, Response};

use crate::{game::LANGUAGES, Config};

pub const GITHUB_BASE_URL: &str = "https://api.github.com";

/// The relevant fields from the gist schema returned by the Github API.
#[derive(Deserialize)]
pub struct Gist {
    pub files: BTreeMap<String, GistFile>,
}

// The relevant fields from the gist file schema returned by the Github API.
#[derive(Deserialize)]
pub struct GistFile {
    pub filename: String,
    pub language: Option<String>,
    pub raw_url: String,
}

/// All the data needed for a round of the game.
pub struct GistData {
    pub url: String,
    pub extension: String,
    pub language: String,
}

impl GistData {
    /// Create a new GistData struct from a [`Gist`]. This will return [`None`]
    /// if none of the gist files use one of the supported languages.
    pub fn from(gist: Gist, syntaxes: &SyntaxSet) -> Option<GistData> {
        let file = gist.files.into_values().find(|file| {
            matches!(file.language.as_ref(), Some(language) if LANGUAGES.contains(&language.as_str()))
        })?;

        let extension = Path::new(&file.filename).extension()?.to_str()?;
        syntaxes.find_syntax_by_extension(extension)?;

        Some(Self {
            url: file.raw_url.to_string(),
            extension: extension.to_string(),
            language: file.language.unwrap(),
        })
    }
}

pub struct Github {
    pub agent: Agent,
    pub token: Option<String>,
}

impl Default for Github {
    fn default() -> Self {
        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
        let user_agent =
            format!("guess-that-lang/{version} (https://github.com/Lioness100/guess-that-lang)");

        Self {
            agent: AgentBuilder::new().user_agent(&user_agent).build(),
            token: None,
        }
    }
}

impl Github {
    pub fn new(config: &mut Config, token: Option<String>) -> anyhow::Result<Self> {
        let mut github = Self::default();
        github.token = github.apply_token(config, token)?;

        Ok(github)
    }

    pub fn apply_token(
        &mut self,
        config: &mut Config,
        token_option: Option<String>,
    ) -> anyhow::Result<Option<String>> {
        if let Some(token) = token_option {
            Github::test_token_structure(&token)?;
            self.validate_token(&token)
                .context("Invalid personal access token")?;

            config.token = token.clone();
            confy::store("guess-that-lang", config)?;
            return Ok(Some(token));
        } else if !config.token.is_empty() {
            let result = self.validate_token(&config.token);
            if result.is_err() {
                config.token = String::new();
                confy::store("guess-that-lang", config)?;
                result.context("The token found in the config is invalid, so it has been removed. Please try again.")?;
            } else {
                return Ok(Some(config.token.clone()));
            }
        }

        Ok(None)
    }

    /// Test a Github personal access token via regex and return it if valid. The
    /// second step of validation is [`validate_token`] which requires querying the
    /// Github API asynchronously and thus can not be used with [`clap::value_parser`].
    pub fn test_token_structure(token: &str) -> anyhow::Result<String> {
        let re = RegexBuilder::new(r"[\da-f]{40}|ghp_\w{36,251}")
            // This is an expensive regex, so the size limit needs to be increased.
            .size_limit(1 << 25)
            .build()
            .unwrap();

        if re.is_match(token) {
            Ok(token.to_string())
        } else {
            bail!("Invalid token format")
        }
    }

    /// Queries the Github ratelimit API using the provided token to make sure it's
    /// valid. The ratelimit data itself isn't used.
    pub fn validate_token(&self, token: &str) -> anyhow::Result<Response> {
        self.agent
            .get(&format!("{GITHUB_BASE_URL}/rate_limit"))
            .set("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(std::convert::Into::into)
    }

    /// Get a vec of random valid gists on Github. This is used with the assumption
    /// that at least one valid gist will be found.
    pub fn get_gists(&self, syntaxes: &SyntaxSet) -> anyhow::Result<Vec<GistData>> {
        let mut request = ureq::get(&format!("{GITHUB_BASE_URL}/gists/public"))
            .query("page", &rand::thread_rng().gen_range(0..=100).to_string());

        if let Some(token) = &self.token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }

        let mut gists = request
            .call()?
            .into_json::<Vec<Gist>>()?
            .into_iter()
            .filter_map(|gist| GistData::from(gist, syntaxes))
            .collect::<Vec<_>>();

        gists.shuffle(&mut thread_rng());

        Ok(gists)
    }

    /// Get single gist content.
    pub fn get_gist(&self, url: &str) -> anyhow::Result<String> {
        let mut request = ureq::get(url);

        if let Some(token) = &self.token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }

        Ok(request.call()?.into_string()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_token_regex() {
        assert!(Github::test_token_structure(&"a".repeat(40)).is_ok());
        assert!(Github::test_token_structure(&format!("ghp_{}", "a".repeat(36))).is_ok());
        assert!(Github::test_token_structure(&"g".repeat(40)).is_err());
        assert!(Github::test_token_structure(&"a".repeat(39)).is_err());
        assert!(Github::test_token_structure(&format!("ghp_{}", ".".repeat(36))).is_err());
        assert!(Github::test_token_structure(&format!("ghp_{}", "a".repeat(35))).is_err());
    }

    #[allow(dead_code)]
    #[ignore]
    fn invalid_token() {
        assert!(Github::new(&mut Config::default(), None)
            .unwrap()
            .validate_token("invalid")
            .is_err());
    }
}
