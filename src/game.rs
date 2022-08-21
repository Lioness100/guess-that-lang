use std::{
    io::stdout,
    ops::ControlFlow,
    sync::{
        mpsc::{self, Receiver},
        Mutex,
    },
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{MoveTo, Show},
    execute,
    style::Stylize,
    terminal::{disable_raw_mode, Clear, ClearType, LeaveAlternateScreen},
};
use rand::{seq::SliceRandom, thread_rng};

use crate::{
    providers::{gists::GistProvider, repos::RepositoryProvider, GithubProvider},
    terminal::Terminal,
    Config, Result, ARGS, CONFIG,
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
    pub provider: Box<dyn GithubProvider>,
}

/// Cleanup terminal after the Game is over (this will also account for
/// unexpected errors).
impl Drop for Game {
    fn drop(&mut self) {
        let _raw = disable_raw_mode();
        let _leave = execute!(self.terminal.stdout, Show, LeaveAlternateScreen);

        println!(
            "\nYou scored {} points!",
            self.points.to_string().green().bold()
        );

        if self.points > CONFIG.high_score {
            if CONFIG.high_score > 0 {
                println!(
                    "You beat your high score of {}!\n\nShare it: {}",
                    CONFIG.high_score.to_string().magenta().bold(),
                    "https://github.com/Lioness100/guess-that-lang/discussions/6"
                        .cyan()
                        .bold()
                );
            }

            let new_config = Config {
                high_score: self.points,
                ..CONFIG.clone()
            };

            let _config = confy::store("guess-that-lang", new_config);
        }
    }
}

impl Game {
    /// Create new game.
    pub fn new() -> Result<Self> {
        let provider: Box<dyn GithubProvider> = match ARGS
            .provider
            .as_ref()
            .unwrap_or(&String::from("repos"))
            .as_str()
        {
            "gists" => Box::new(GistProvider::new()?),
            "repos" => Box::new(RepositoryProvider::new()?),
            _ => return Err("Invalid github provider (repos/gists)".into()),
        };

        Ok(Self {
            points: 0,
            terminal: Terminal::new()?,
            provider,
        })
    }

    /// Get the language options for a round. This will choose 3 random unique
    /// languages, push them to a vec along with the correct language, and
    /// shuffle the vec.
    #[must_use]
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
    pub fn start_new_round(&mut self, preloader: Option<Receiver<()>>) -> Result<ControlFlow<()>> {
        let data = self.provider.get_code()?;
        let width = Terminal::width()?;

        let highlighter = self.terminal.get_highlighter(&data.language);
        let code = match self.terminal.parse_code(&data.code, highlighter, &width) {
            Some(code) => code,
            // If there is no valid code, skip this round via recursion.
            None => return self.start_new_round(preloader),
        };

        let options = Self::get_options(&data.language);

        if let Some(preloader) = preloader {
            let _ = preloader.recv();
        }

        self.terminal
            .print_round_info(&options, &code, &width, self.points)?;

        let available_points = Mutex::new(100.0);
        let (sender, receiver) = mpsc::channel();

        // [`Terminal::start_showing_code`] and [`Terminal::read_input_char`]
        // both create blocking loops, so they have to be used in separate threads.
        thread::scope(|s| {
            let display = s.spawn(|| {
                self.terminal
                    .start_showing_code(&code, &available_points, receiver)
            });

            let input = s.spawn(|| {
                let input = Terminal::read_input_char()?;

                // Notifies [`Terminal::start_showing_code`] to not show the
                // next line.
                let sender = sender;
                let _ = sender.send(());

                if input == 'q' || input == 'c' {
                    Ok(ControlFlow::Break(()))
                } else {
                    let result = self.terminal.process_input(
                        input.to_digit(10).ok_or("invalid input")?,
                        &options,
                        &data.language,
                        &available_points,
                        &mut self.points,
                    );

                    // Let the user visually process the result. If they got it
                    // correct, the timer is set after a thread is spawned to
                    // preload the next round's gist.
                    if let Ok(ControlFlow::Break(())) = result {
                        thread::sleep(Duration::from_millis(1500));
                    }

                    result
                }
            });

            display.join().unwrap()?;
            input.join().unwrap()
        })
    }

    /// Wait 1.5 seconds for the user to visually process they got the right
    /// answer while the next round is preloading, then start the next round.
    pub fn start_next_round(&mut self) -> Result<ControlFlow<()>> {
        let (sender, receiver) = mpsc::channel();

        thread::scope(|s| {
            let handle = s.spawn(|| self.start_new_round(Some(receiver)));

            thread::sleep(Duration::from_millis(1500));

            // Clear the screen and move to the top right corner. This is not
            // a method of [`Terminal`] because it would take a lot of work to
            // let the borrow checker let me use `self` again.
            let _clear = execute!(stdout().lock(), Clear(ClearType::All), MoveTo(0, 0));
            let _ = sender.send(());

            handle.join().unwrap()
        })
    }
}
