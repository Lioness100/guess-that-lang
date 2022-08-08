use std::{collections::BTreeMap, path::Path};

use anyhow::bail;
use octocrab::OctocrabBuilder;
use rand::{seq::SliceRandom, thread_rng, Rng};
use regex::RegexBuilder;
use serde::Deserialize;
use syntect::parsing::SyntaxSet;

use crate::{game::LANGUAGES, CONFIG_PATH};

/// Test a Github personal access token via regex and return it if valid. The
/// second step of validation is [`validate_token`] which requires querying the
/// Github API asynchronously and thus can not be used with [`clap::value_parser`].
pub fn test_token_structure(token: &str) -> Result<String, String> {
    let re = RegexBuilder::new(r"[\da-f]{40}|ghp_\w{36,251}")
        // This is an expensive regex, so the size limit needs to be increased.
        .size_limit(1 << 25)
        .build()
        .unwrap();

    if re.is_match(token) {
        Ok(token.to_string())
    } else {
        Err(String::from("Invalid access token"))
    }
}

/// Queries the Github ratelimit API using the provided token to make sure it's
/// valid. The ratelimit data itself isn't used.
pub async fn apply_token(token: &str, from_file: bool) -> anyhow::Result<()> {
    // Register the token as authentication with [`octocrab::Octocrab`].
    octocrab::initialise(OctocrabBuilder::new().personal_token(token.to_string()))?;
    let ratelimit = octocrab::instance().ratelimit().get().await;

    if ratelimit.is_err() {
        bail!(
            "Invalid personal access token{}",
            if from_file {
                format!(
                    " (from {}). Please delete the file and try again.",
                    CONFIG_PATH
                )
            } else {
                String::from("")
            }
        );
    }

    Ok(())
}

/// The relevant fields from the gist schema returned by the Github API.
#[derive(Debug, Deserialize)]
pub struct Gist {
    pub files: BTreeMap<String, GistFile>,
}

// The relevant fields from the gist file schema returned by the Github API.
#[derive(Debug, Deserialize)]
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
    fn from(gist: Gist, syntaxes: &SyntaxSet) -> Option<GistData> {
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

/// Get a vec of random valid gists on Github. This is used with the assumption
/// that at least one valid gist will be found.
pub async fn get_gists(syntaxes: &SyntaxSet) -> anyhow::Result<Vec<GistData>> {
    let octocrab = octocrab::instance();
    let relative_url = format!(
        "gists/public?page={}",
        rand::thread_rng().gen_range(0..=100)
    );

    let gists_page = octocrab
        .get_page::<Gist>(&Some(octocrab.absolute_url(relative_url).unwrap()))
        .await?
        .unwrap();

    let mut valid_gists = gists_page
        .into_iter()
        .filter_map(|gist| GistData::from(gist, syntaxes))
        .collect::<Vec<_>>();

    valid_gists.shuffle(&mut thread_rng());

    Ok(valid_gists)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_token_regex() {
        assert!(test_token_structure(&"a".repeat(40)).is_ok());
        assert!(test_token_structure(&format!("ghp_{}", "a".repeat(36))).is_ok());
        assert!(test_token_structure(&"g".repeat(40)).is_err());
        assert!(test_token_structure(&"a".repeat(39)).is_err());
        assert!(test_token_structure(&format!("ghp_{}", ".".repeat(36))).is_err());
        assert!(test_token_structure(&format!("ghp_{}", "a".repeat(35))).is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn invalid_token() {
        assert!(apply_token("invalid", false).await.is_err());
    }
}
