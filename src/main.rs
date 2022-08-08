use anyhow::Context;
use tokio::fs;

use clap::Parser;
use game::GameResult;

pub mod game;
pub mod github;
pub mod terminal;

use crate::{
    game::Game,
    github::{apply_token, test_token_structure},
};

pub const CONFIG_PATH: &str = ".guess-that-lang";

/// Struct used to resolve CLI arguments.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(
        long,
        help = "Your personal access token, which will be stored in the .guess-that-lang file and thus will only need to be input once. This will allow the game to make more Github requests before getting ratelimited.\nNo scopes are required: https://github.com/settings/tokens/new?description=Guess%20That%20Lang",
        value_parser = test_token_structure
    )]
    token: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Some(token) = args.token {
        apply_token(&token, false).await?;
        fs::write(CONFIG_PATH, token)
            .await
            .context("Failed to write token to config file")?;
    } else if let Ok(token) = fs::read_to_string(CONFIG_PATH).await {
        apply_token(&token, true).await?;
    }

    let mut game = Game::new().await?;
    loop {
        let result = game.start_new_round().await?;
        match result {
            GameResult::Continue => game.terminal.clear_screen(),
            GameResult::Exit => break,
        };
    }

    Ok(())
}
