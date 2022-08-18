#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::cargo)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::{error::Error, ops::ControlFlow, result};

use argh::FromArgs;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

pub mod game;
pub mod github;
pub mod terminal;

use crate::{game::Game, github::Github, terminal::ThemeStyle};

pub type Result<T> = result::Result<T, Box<dyn Error + Send + Sync>>;

/// CLI game to see how fast you can guess the language of a code block!
#[derive(FromArgs)]
pub struct Args {
    /// your personal access token
    #[argh(short = 't', option)]
    token: Option<String>,

    /// the number of ms to wait before revealing code
    #[argh(short = 'w', option)]
    wait: Option<u64>,

    /// whether or not to reveal lines in random order
    #[argh(short = 's', switch)]
    shuffle: bool,

    /// whether to use dark or light theme (dark/light)
    #[argh(option)]
    theme: Option<String>,
}

/// Values to be persisted in a .toml file.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Config {
    high_score: u32,
    token: String,
    theme: Option<ThemeStyle>,
    delay: Option<u64>,
}

pub const DEFAULT_INITIAL_DELAY: u64 = 1500;
pub const CODE_DELAY: u64 = 1500;
lazy_static! {
    pub static ref ARGS: Args = argh::from_env();
    pub static ref CONFIG: Config = confy::load("guess-that-lang").unwrap();
}

pub fn main() -> Result<()> {
    let client = Github::new()?;
    let mut game = Game::new(client)?;

    let mut result = game.start_new_round(None)?;

    while let ControlFlow::Continue(_) = result {
        result = game.start_next_round()?;
    }

    Ok(())
}
