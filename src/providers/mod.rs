use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};
use ureq::{Agent, AgentBuilder, Request, Response};

use crate::{Config, Result, ARGS, CONFIG};

pub mod gists;
pub mod repos;

pub const GITHUB_BASE_URL: &str = "https://api.github.com";

lazy_static! {
    static ref TOKEN_REGEX: Regex = RegexBuilder::new(r"[\da-f]{40}|ghp_\w{36,251}")
        // This is an expensive regex, so the size limit needs to be increased.
        .size_limit(1 << 25)
        .build()
        .unwrap();
}

pub struct CodeData {
    pub code: String,
    pub language: String,
}

pub trait GithubProvider: Send {
    fn new() -> Result<Self>
    where
        Self: Sized;

    fn get_code(&mut self) -> Result<CodeData>;

    #[must_use]
    fn get_agent() -> Agent
    where
        Self: Sized,
    {
        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
        let user_agent =
            format!("guess-that-lang/{version} (https://github.com/Lioness100/guess-that-lang)");

        AgentBuilder::new().user_agent(&user_agent).build()
    }

    /// If a token is found from arguments or the config: validate it and return
    /// it. If it wasn't found from the config, store it in the config.
    fn apply_token(agent: &Agent) -> Result<Option<String>>
    where
        Self: Sized,
    {
        if let Some(token) = &ARGS.token {
            Self::test_token_structure(token)?;

            if Self::validate_token(agent, token).is_err() {
                return Err("Invalid personal access token".into());
            }

            confy::store(
                "guess-that-lang",
                Config {
                    token: token.clone(),
                    ..CONFIG.clone()
                },
            )?;

            return Ok(Some(token.to_string()));
        }

        if !CONFIG.token.is_empty() {
            let result = Self::validate_token(agent, &CONFIG.token);
            if result.is_err() {
                confy::store(
                    "guess-that-lang",
                    Config {
                        token: String::new(),
                        ..CONFIG.clone()
                    },
                )?;

                return Err("The token found in the config is invalid, so it has been removed. Please try again.".into());
            }

            return Ok(Some(CONFIG.token.clone()));
        }

        Ok(None)
    }

    /// Test a Github personal access token via regex.
    fn test_token_structure(token: &str) -> Result<()>
    where
        Self: Sized,
    {
        if TOKEN_REGEX.is_match(token) {
            Ok(())
        } else {
            Err("Invalid personal access token".into())
        }
    }

    /// Queries the Github ratelimit API using the provided token to make sure it's
    /// valid. The ratelimit data itself isn't used.
    fn validate_token<S: AsRef<str>>(agent: &Agent, token: S) -> Result<Response>
    where
        Self: Sized,
    {
        agent
            .get(&format!("{GITHUB_BASE_URL}/rate_limit"))
            .with_authentication(Some(token))
            .call()
            .map_err(Into::into)
    }
}

pub trait AuthenticationExt {
    #[must_use]
    fn with_authentication<S: AsRef<str>>(self, token: Option<S>) -> Self;
}

impl AuthenticationExt for Request {
    fn with_authentication<S: AsRef<str>>(self, token: Option<S>) -> Self {
        match token {
            Some(token) => self.set("Authorization", &format!("Bearer {}", token.as_ref())),
            None => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider;

    impl GithubProvider for TestProvider {
        fn new() -> Result<Self> {
            Ok(Self {})
        }

        fn get_code(&mut self) -> Result<CodeData> {
            Ok(CodeData {
                code: String::from(""),
                language: String::from(""),
            })
        }
    }

    #[test]
    fn access_token_regex() {
        assert!(TestProvider::test_token_structure(&"a".repeat(40)).is_ok());
        assert!(TestProvider::test_token_structure(&"g".repeat(40)).is_err());
        assert!(TestProvider::test_token_structure(&"a".repeat(39)).is_err());
        assert!(TestProvider::test_token_structure(&format!("ghp_{}", ".".repeat(36))).is_err());
        assert!(TestProvider::test_token_structure(&format!("ghp_{}", "a".repeat(35))).is_err());
        assert!(TestProvider::test_token_structure(&format!("ghp_{}", "a".repeat(36))).is_ok());
    }

    #[allow(dead_code)]
    #[ignore]
    fn invalid_token() {
        assert!(TestProvider::validate_token(&TestProvider::get_agent(), "invalid").is_err());
    }
}
