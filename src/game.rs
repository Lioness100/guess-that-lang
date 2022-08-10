use std::{
    ops::ControlFlow,
    sync::{mpsc, Mutex},
    thread,
    time::Duration,
};

use ansi_term::Color;
use crossterm::{
    cursor::Show,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use rand::{seq::SliceRandom, thread_rng};

use crate::{
    github::{GistData, Github},
    terminal::Terminal,
    Config,
};

/// The prompt to be shown before the options in [`Terminal::print_round_info`].
pub const PROMPT: &str = "Which programming language is this? (Type the corresponding number)";

/// All valid languages (top 24 from the Stack Overflow 2022 Developer survey,
/// but substituting VBA for Dockerfile).
pub const LANGUAGES: [&str; 25] = [
    "Assembly",
    "Shell",
    "C",
    "C#",
    "C++",
    "CSS",
    "Dart",
    "Dockerfile",
    "Go",
    "Groovy",
    "HTML",
    "Java",
    "JavaScript",
    "Kotlin",
    "Lua",
    "MATLAB",
    "PHP",
    "PowerShell",
    "Python",
    "R",
    "Ruby",
    "Rust",
    "SQL",
    "Swift",
    "TypeScript",
];

/// The all-encompassing game struct.
pub struct Game {
    pub points: u32,
    pub terminal: Terminal,
    pub config: Config,
    pub client: Github,
    pub gist_data: Vec<GistData>,
}

/// Cleanup terminal after the Game is over (this will also account for
/// unexpected errors).
impl Drop for Game {
    fn drop(&mut self) {
        let _raw = disable_raw_mode();
        let _leave = execute!(self.terminal.stdout, Show, LeaveAlternateScreen);

        println!(
            "\nYou scored {} points!",
            Color::Green.bold().paint(self.points.to_string())
        );

        if self.points > self.config.high_score {
            if self.config.high_score > 0 {
                println!(
                    "You beat your high score of {}!",
                    Color::Green
                        .bold()
                        .paint(self.config.high_score.to_string())
                );
            }

            let new_config = Config {
                high_score: self.points,
                token: self.config.token.clone(),
            };

            let _config = confy::store("guess-that-lang", new_config);
        }
    }
}

impl Game {
    /// Create new game.
    pub fn new(config: Config, client: Github) -> Self {
        Self {
            points: 0,
            terminal: Terminal::default(),
            gist_data: Vec::new(),
            client,
            config,
        }
    }

    /// Get the language options for a round. This will choose 3 random unique
    /// languages, push them to a vec along with the correct language, and
    /// shuffle the vec.
    pub fn get_options(correct_language: &str) -> Vec<&str> {
        let mut options = Vec::<&str>::with_capacity(4);
        options.push(correct_language);

        let mut thread_rng = thread_rng();
        while options.len() < 4 {
            let random_language = LANGUAGES.choose(&mut thread_rng).unwrap();
            if !options.contains(random_language) {
                options.push(random_language);
            }
        }

        options.shuffle(&mut thread_rng);
        options
    }

    /// Start a new round, which is called in the main function with a for loop.
    pub fn start_new_round(&mut self) -> anyhow::Result<ControlFlow<()>> {
        if self.gist_data.is_empty() {
            self.gist_data = self.client.get_gists(&self.terminal.syntaxes)?;
        }

        let gist = self.gist_data.pop().unwrap();
        let code = self.client.get_gist(&gist.url)?;

        let options = Self::get_options(&gist.language);
        self.terminal.print_round_info(
            &self.config,
            self.points,
            &options,
            Terminal::trim_code(&code),
        );

        let available_points = Mutex::new(100.0);
        let (sender, receiver) = mpsc::channel();

        // [`Terminal::start_showing_code`] and [`Terminal::read_input_char`]
        // both create blocking loops, so they have to be used in separate threads.
        crossbeam_utils::thread::scope(|s| {
            let display = s.spawn(|_| {
                self.terminal.start_showing_code(
                    Terminal::trim_code(&code),
                    &gist.extension,
                    &available_points,
                    receiver,
                );
            });

            let input = s.spawn(|_| {
                let char = Terminal::read_input_char();
                if char == 'q' {
                    sender.send(()).unwrap();
                    Ok(ControlFlow::Break(()))
                } else {
                    let result = self.terminal.process_input(
                        sender,
                        char.to_digit(10).unwrap(),
                        &options,
                        &gist.language,
                        &available_points,
                        &mut self.points,
                    );

                    // Give the user 1.5 seconds to register the result.
                    thread::sleep(Duration::from_millis(1500));
                    result
                }
            });

            display.join().unwrap();
            input.join().unwrap()
        })
        .unwrap()
    }
}
