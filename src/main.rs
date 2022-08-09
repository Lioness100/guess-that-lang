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

use argh::FromArgs;
use game::GameResult;

pub mod game;
pub mod github;
pub mod path;
pub mod terminal;

use crate::{game::Game, github::Github};

pub const CONFIG_PATH: &str = ".guess-that-lang";

#[derive(FromArgs)]
/// CLI game to see how fast you can guess the language of a code block!
struct Args {
    /// your personal access token, which will be stored in the .guess-that-lang file and thus will only need to be input once. This will allow the game to make more Github requests before getting ratelimited.
    /// No scopes are required: https://github.com/settings/tokens/new?description=Guess%20That%20Lang
    #[argh(short = 't', option)]
    token: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    let mut client = Github::default();
    client.apply_token(args.token)?;

    let mut game = Game::new(client);
    loop {
        let result = game.start_new_round()?;
        match result {
            GameResult::Continue => game.terminal.clear_screen(),
            GameResult::Exit => break,
        };
    }

    Ok(())
}
