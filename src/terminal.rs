use std::{
    env,
    io::{stdout, Stdout, Write},
    ops::ControlFlow,
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
    thread,
    time::Duration,
};

#[cfg(windows)]
use ansi_term::enable_ansi_support;

use ansi_colours::ansi256_from_rgb;
use ansi_term::{
    ANSIGenericStrings,
    Color::{self, Fixed, RGB},
};
use anyhow::Context;
use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, RestorePosition, SavePosition},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{enable_raw_mode, Clear, ClearType, EnterAlternateScreen},
};
use rand::{seq::SliceRandom, thread_rng};
// Lioness100/guess-that-lang#5
// use dark_light::Mode;
use syntect::{
    dumps,
    easy::HighlightLines,
    highlighting::{self, Theme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use crate::{game::PROMPT, Args, Config};

pub struct Terminal {
    pub syntaxes: SyntaxSet,
    pub stdout: Stdout,
    pub theme: Theme,
    pub is_truecolor: bool,
}

impl Default for Terminal {
    fn default() -> Self {
        #[cfg(windows)]
        let _ansi = enable_ansi_support();

        let themes: ThemeSet = dumps::from_binary(include_bytes!("../assets/dumps/themes.dump"));
        let syntaxes: SyntaxSet =
            dumps::from_uncompressed_data(include_bytes!("../assets/dumps/syntaxes.dump"))
                .context("Failed to load syntaxes")
                .unwrap();

        let mut stdout = stdout();

        let _hide = execute!(stdout, EnterAlternateScreen, Hide);
        let _raw = enable_raw_mode();

        Self {
            syntaxes,
            stdout,
            theme: themes.themes[Terminal::get_theme()].clone(),
            is_truecolor: Terminal::is_truecolor(),
        }
    }
}

impl Terminal {
    /// Highlight a line of code.
    pub fn highlight_line(&self, code: &str, highlighter: &mut HighlightLines) -> String {
        let ranges = highlighter.highlight_line(code, &self.syntaxes).unwrap();
        let mut colorized = Vec::with_capacity(ranges.len());

        for (style, component) in ranges {
            let color = Self::to_ansi_color(style.foreground, self.is_truecolor);
            colorized.push(color.paint(component));
        }

        ANSIGenericStrings(&colorized).to_string()
    }

    /// Converts [`syntect::highlighting::Color`] to [`ansi_term::Color`]. The
    /// implementation is taked from https://github.com/sharkdp/bat and relevant
    /// explanations of this functions can be found there.
    pub fn to_ansi_color(color: highlighting::Color, true_color: bool) -> ansi_term::Color {
        if color.a == 0 {
            match color.r {
                0x00 => Color::Black,
                0x01 => Color::Red,
                0x02 => Color::Green,
                0x03 => Color::Yellow,
                0x04 => Color::Blue,
                0x05 => Color::Purple,
                0x06 => Color::Cyan,
                0x07 => Color::White,
                n => Fixed(n),
            }
        } else if true_color {
            RGB(color.r, color.g, color.b)
        } else {
            Fixed(ansi256_from_rgb((color.r, color.g, color.b)))
        }
    }

    /// Return true if the current running terminal support true color.
    pub fn is_truecolor() -> bool {
        env::var("COLORTERM")
            .map(|colorterm| colorterm == "truecolor" || colorterm == "24bit")
            .unwrap_or(false)
    }

    /// Get light/dark mode specific theme.
    pub fn get_theme() -> &'static str {
        "Monokai Extended"
        // Lioness100/guess-that-lang#5
        // match dark_light::detect() {
        //     Mode::Dark => "Monokai Extended",
        //     Mode::Light => "Monakai Extended Light",
        // }
    }

    /// Cuts the code horizontally after 10 non empty lines and vertically after
    /// it exceeds the terminal width. Returns an iterator (with indeces for
    /// line numbers). This is used for both [`Terminal::print_round_info`] and
    /// [`Terminal::start_showing_code`].
    pub fn trim_code<'a>(
        code: &'a str,
        width: &'a usize,
    ) -> impl Iterator<Item = (usize, String)> + 'a {
        let mut taken_lines: u8 = 0;
        LinesWithEndings::from(code)
            .take_while(move |&line| {
                if line == "\n" {
                    true
                } else {
                    taken_lines += 1;
                    taken_lines <= 10
                }
            })
            .map(|line| {
                if line.len() + 9 > *width {
                    format!("{}...", &line[..*width - 12])
                } else {
                    line.to_string()
                }
            })
            .enumerate()
    }

    /// Print the base table and all elements inside, including the code in dot form.
    pub fn print_round_info(
        &self,
        config: &Config,
        points: u32,
        options: &[&str],
        code_lines: impl Iterator<Item = (usize, String)>,
        width: &usize,
    ) {
        let pipe = Color::White.dimmed().paint("│");

        let points = format!(
            "{padding}{pipe} {}{}\r\n{padding}{pipe} {}{}\r\n{padding}{pipe} {}{}",
            Color::White.bold().paint("High Score: "),
            Color::Purple.paint(config.high_score.to_string()),
            Color::White.bold().paint("Total Points: "),
            Color::Cyan.paint(points.to_string()),
            Color::White.bold().paint("Available Points: "),
            Color::RGB(0, 255, 0).paint("100"),
            padding = " ".repeat(7),
        );

        let line_separator_start = "─".repeat(7);
        let line_separator_end = "─".repeat(width - 8);

        let [top, mid, bottom] = ["┬", "┼", "┴"].map(|char| {
            Color::White
                .dimmed()
                .paint(line_separator_start.clone() + char + &line_separator_end)
                .to_string()
        });

        let dotted_code = code_lines
            .map(|(idx, line)| {
                let dots: String = line
                    .chars()
                    // Replace all non whitespace characters with dots.
                    .map(|char| if char.is_whitespace() { char } else { '·' })
                    .collect();

                // Trim the end of the line to remove extraneous newlines, and
                // then add one manually.
                format!("{: ^7}{pipe} {}\r\n", idx + 1, dots.trim_end())
            })
            .collect::<String>();

        let option_text = options
            .iter()
            .enumerate()
            .map(|(idx, option)| Self::format_option(&(idx + 1).to_string(), option))
            .collect::<Vec<_>>()
            .join("\r\n");

        let quit_option_text = Self::format_option("q", "Quit");

        let text = format!(
            "{top}\r\n{points}\r\n{mid}\r\n{dotted_code}{bottom}\r\n\r\n{PROMPT}\r\n\r\n{option_text}\r\n{quit_option_text}"
        );

        execute!(self.stdout.lock(), Print(text)).unwrap();
    }

    /// Create a loop that will reveal a line of code and decrease
    /// `available_points` every 1.5 seconds.
    pub fn start_showing_code(
        &self,
        code_lines: impl Iterator<Item = (usize, String)>,
        extension: &str,
        available_points: &Mutex<f32>,
        args: &Args,
        receiver: Receiver<()>,
    ) {
        let syntax = self
            .syntaxes
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);

        let iter: Box<dyn Iterator<Item = (usize, String)>> = if args.shuffle {
            let mut code_lines_vec = code_lines.collect::<Vec<_>>();
            code_lines_vec.shuffle(&mut thread_rng());

            Box::new(code_lines_vec.into_iter())
        } else {
            Box::new(code_lines)
        };

        // This has to be made a variable as opposed to just checking if idx ==
        // 0 because the lines could be shuffled.
        let mut is_first_line = false;
        for (idx, line) in iter {
            if line == "\n" {
                continue;
            }

            let millis = if is_first_line { args.wait } else { 1500 };
            is_first_line = false;

            thread::sleep(Duration::from_millis(millis));

            // The receiver will receive a message when the user has selected an
            // option, at which point the code should not be updated further.
            if receiver.try_recv().is_ok() {
                break;
            }

            let mut stdout = self.stdout.lock();
            let line = self.highlight_line(&line, &mut highlighter);

            // Move to the row index of the dotted code and replace it with the
            // real code.
            queue!(stdout, SavePosition, MoveTo(9, idx as u16 + 5), Print(line)).unwrap();

            // `available_points` should not be decreased on the first line.
            if idx != 0 {
                let mut available_points = available_points.lock().unwrap();
                *available_points -= 10.0;

                // https://stackoverflow.com/a/7947812/13721990
                let new_color = Color::RGB(
                    255.0_f32.min(255.0 * 2.0 * (1.0 - (*available_points / 100.0))) as u8,
                    255.0_f32.min(2.0 * 255.0 * (*available_points / 100.0)) as u8,
                    0,
                );

                queue!(
                    stdout,
                    MoveTo(27, 3),
                    Print(format!(
                        "{} ",
                        new_color.paint(available_points.to_string())
                    ))
                )
                .unwrap();
            }

            execute!(stdout, RestorePosition).unwrap();
        }
    }

    /// Responds to input from the user (1 | 2 | 3 | 4).
    pub fn process_input(
        &self,
        sender: Sender<()>,
        num: u32,
        options: &[&str],
        correct_language: &str,
        available_points: &Mutex<f32>,
        total_points: &mut u32,
    ) -> anyhow::Result<ControlFlow<()>> {
        // Send a message which results in [`Terminal::start_showing_code`] to
        // not show the next line.
        let _ = sender.send(());

        // Locking the stdout will let any work that's being done in
        // [`Terminal::start_showing_code`] to finish before we continue.
        let mut stdout = self.stdout.lock();

        let correct_option_idx = options
            .iter()
            .position(|&option| option == correct_language)
            .unwrap();

        let was_correct = (correct_option_idx + 1) as u32 == num;
        let available_points = available_points.lock().unwrap();

        let correct_option_name_text = if was_correct {
            format!("{correct_language} (+ {available_points})")
        } else {
            format!("{correct_language} (Correct)")
        };

        let correct_option_text = Self::format_option(
            &(correct_option_idx + 1).to_string(),
            &Color::Green
                .bold()
                .paint(correct_option_name_text)
                .to_string(),
        );

        queue!(
            stdout,
            SavePosition,
            MoveUp((4 - correct_option_idx) as u16),
            MoveToColumn(0),
            Print(correct_option_text),
            RestorePosition
        )?;

        if was_correct {
            *total_points += *available_points as u32;
            stdout.flush()?;

            Ok(ControlFlow::Continue(()))
        } else {
            let incorrect_option_text = Self::format_option(
                &num.to_string(),
                &Color::RGB(255, 0, 51)
                    .bold()
                    .paint(format!("{} (Incorrect)", options[num as usize - 1]))
                    .to_string(),
            );

            execute!(
                stdout,
                SavePosition,
                MoveUp((5 - num) as u16),
                MoveToColumn(0),
                Print(incorrect_option_text),
                RestorePosition
            )?;

            Ok(ControlFlow::Break(()))
        }
    }

    /// Utility function to wait for a relevant char to be pressed.
    pub fn read_input_char() -> char {
        loop {
            if let Ok(Event::Key(KeyEvent {
                code: KeyCode::Char(char @ ('1' | '2' | '3' | '4' | 'q' | 'c')),
                modifiers,
                ..
            })) = event::read()
            {
                if char == 'c' && modifiers != KeyModifiers::CONTROL {
                    continue;
                }

                return char;
            }
        }
    }

    /// Utility function to format an option.
    pub fn format_option(key: &str, name: &str) -> String {
        format!(
            "{padding}[{key}] {name}",
            padding = " ".repeat(5),
            key = Color::White.bold().paint(key),
            name = Color::White.paint(name)
        )
    }

    /// Clear the screen and move to the top right corner. This is done at the
    /// start of each round.
    pub fn clear_screen(&self) {
        execute!(self.stdout.lock(), Clear(ClearType::All), MoveTo(0, 0)).unwrap();
    }
}
