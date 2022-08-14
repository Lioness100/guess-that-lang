#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(
    // Allowed to avoid breaking changes.
    clippy::module_name_repetitions,
    clippy::struct_excessive_bools,
    clippy::unused_self,
    // Allowed as they are too pedantic
    clippy::cast_possible_truncation,
    clippy::unreadable_literal,
    clippy::cast_possible_wrap,
    clippy::wildcard_imports,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::doc_markdown,
    clippy::cast_lossless,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    // Document this later
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
)]

use std::ops::ControlFlow;

use argh::FromArgs;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

pub mod game;
pub mod github;
pub mod terminal;

use crate::{game::Game, github::Github, terminal::ThemeStyle};

/// CLI game to see how fast you can guess the language of a code block!
#[derive(FromArgs)]
pub struct Args {
    /// your personal access token
    #[argh(short = 't', option)]
    token: Option<String>,

    /// the number of ms to wait before revealing code
    #[argh(short = 'w', option, default = "1500")]
    wait: u64,

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
}

lazy_static! {
    pub static ref ARGS: Args = argh::from_env();
    pub static ref CONFIG: Config = confy::load("guess-that-lang").unwrap();
}

pub fn main() -> anyhow::Result<()> {
    let client = Github::new()?;
    let mut game = Game::new(client)?;

    loop {
        let result = game.start_new_round()?;
        match result {
            ControlFlow::Continue(_) => game.terminal.clear_screen(),
            ControlFlow::Break(_) => break,
        };
    }

    Ok(())
}
