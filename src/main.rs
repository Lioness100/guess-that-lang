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
use serde::{Deserialize, Serialize};

pub mod game;
pub mod github;
pub mod terminal;

use crate::{game::Game, github::Github};

/// CLI game to see how fast you can guess the language of a code block!
#[derive(FromArgs)]
struct Args {
    /// your personal access token, which will be stored in the .guess-that-lang file and thus will only need to be input once. This will allow the game to make more Github requests before getting ratelimited.
    /// No scopes are required: https://github.com/settings/tokens/new?description=Guess%20That%20Lang
    #[argh(short = 't', option)]
    token: Option<String>,
}

/// Values to be persisted in a .toml file.
#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    high_score: u32,
    token: String,
}

pub fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();
    let mut config: Config = confy::load("guess-that-lang")?;

    let client = Github::new(&mut config, args.token)?;
    let mut game = Game::new(config, client);

    loop {
        let result = game.start_new_round()?;
        match result {
            ControlFlow::Continue(_) => game.terminal.clear_screen(),
            ControlFlow::Break(_) => break,
        };
    }

    Ok(())
}
