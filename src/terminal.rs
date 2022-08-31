use std::{
    env,
    io::{stdout, Stdout, Write},
    ops::ControlFlow,
    result,
    sync::{mpsc::Receiver, Mutex},
    time::Duration,
};

#[cfg(windows)]
use ansi_term::enable_ansi_support;

use ansi_colours::ansi256_from_rgb;
use ansi_term::{
    ANSIStrings,
    Color::{self, Fixed, RGB},
};
use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, RestorePosition, SavePosition},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, queue,
    style::{Print, Stylize},
    terminal::{self, enable_raw_mode, EnterAlternateScreen},
};
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use syntect::{
    dumps,
    easy::HighlightLines,
    highlighting::{self, Theme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use crate::{game::PROMPT, Config, Result, ARGS, CONFIG};

#[derive(Serialize, Deserialize, Clone)]
pub enum ThemeStyle {
    Dark,
    Light,
}

impl TryFrom<Option<String>> for ThemeStyle {
    type Error = ();

    fn try_from(opt: Option<String>) -> result::Result<Self, Self::Error> {
        match opt {
            Some(string) if string == "dark" => Ok(Self::Dark),
            Some(string) if string == "light" => Ok(Self::Light),
            _ => Err(()),
        }
    }
}

impl From<ThemeStyle> for &'static str {
    fn from(theme: ThemeStyle) -> Self {
        match theme {
            ThemeStyle::Dark => "Monokai Extended",
            ThemeStyle::Light => "Monokai Extended Light",
        }
    }
}

pub struct Terminal {
    pub syntaxes: SyntaxSet,
    pub stdout: Stdout,
    pub theme: Theme,
    pub is_truecolor: bool,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        #[cfg(windows)]
        let _ = enable_ansi_support();

        let themes: ThemeSet = dumps::from_binary(include_bytes!("../assets/dumps/themes.dump"));
        let syntaxes: SyntaxSet =
            dumps::from_uncompressed_data(include_bytes!("../assets/dumps/syntaxes.dump"))?;

        let mut stdout = stdout();

        if !cfg!(test) {
            let _clear = execute!(stdout, EnterAlternateScreen, Hide, MoveTo(0, 0));
            let _raw = enable_raw_mode();
        }

        Ok(Self {
            syntaxes,
            stdout,
            theme: themes.themes[Self::get_theme()?].clone(),
            is_truecolor: Self::is_truecolor(),
        })
    }

    /// Highlight a line of code.
    pub fn highlight_line(&self, code: &str, highlighter: &mut HighlightLines) -> Option<String> {
        let ranges = highlighter.highlight_line(code, &self.syntaxes).ok()?;
        let mut colorized = Vec::with_capacity(ranges.len());

        for (style, component) in ranges {
            let color = Self::to_ansi_color(style.foreground, self.is_truecolor);

            // This color represents comments. If the line includes a comment,
            // it should be excluded from the output so the user can look at
            // actual code. The second color is for bash specifically because it
            // for some reason has a different comment color.
            if matches!(color, Color::RGB(117, 113, 94) | Color::RGB(124, 120, 101)) {
                return None;
            };

            colorized.push(color.paint(component));
        }

        Some(ANSIStrings(&colorized).to_string())
    }

    /// Converts [`syntect::highlighting::Color`] to [`ansi_term::Color`]. The
    /// implementation is taken from <https://github.com/sharkdp/bat> and relevant
    /// explanations of this functions can be found there.
    #[must_use]
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
    #[must_use]
    pub fn is_truecolor() -> bool {
        env::var("COLORTERM")
            .map(|colorterm| colorterm == "truecolor" || colorterm == "24bit")
            .unwrap_or_default()
    }

    /// Get light/dark mode specific theme.
    pub fn get_theme() -> Result<&'static str> {
        if let Ok(theme) = ThemeStyle::try_from(ARGS.theme.clone()) {
            confy::store(
                "guess-that-lang",
                Config {
                    theme: Some(theme.clone()),
                    ..CONFIG.clone()
                },
            )?;

            Ok(theme.into())
        } else if let Some(theme) = CONFIG.theme.clone() {
            Ok(theme.into())
        } else {
            #[cfg(target_os = "macos")]
            {
                if !macos_dark_mode_active() {
                    return Ok(ThemeStyle::Light.into());
                }
            }

            Ok(ThemeStyle::Dark.into())
        }
    }

    /// Parses the code in a number of ways:
    /// - Cuts the code off after in exceeds the terminal width, replacing the
    ///   last three characters with "..."
    /// - Cuts out all comments
    /// - Cuts the code off after 10 non-empty lines
    /// - Removes all but the first of all consecutive newlines
    /// - Trims leading and trailing newlines
    pub fn parse_code(
        &self,
        code: &str,
        mut highlighter: HighlightLines,
        width: &usize,
    ) -> Option<Vec<(String, String)>> {
        let mut taken_lines: u8 = 0;

        let mut lines: Vec<_> = LinesWithEndings::from(code)
            .filter_map(move |line| {
                let trimmed = if line.len() + 9 > *width {
                    format!("{}...", &line[..*width - 12])
                } else {
                    line.to_owned()
                };

                self.highlight_line(&trimmed, &mut highlighter)
                    .map(|highlighted| (trimmed, highlighted))
            })
            .take_while(move |(line, _)| {
                if line == "\n" {
                    true
                } else {
                    taken_lines += 1;
                    taken_lines <= 10
                }
            })
            .collect();

        lines.dedup_by(|(a, _), (b, _)| a == "\n" && b == "\n");

        let count_end = lines.len()
            - lines
                .iter()
                .rev()
                .take_while(|&(line, _)| line == "\n")
                .count();

        lines.truncate(count_end);

        if lines.is_empty() {
            return None;
        }

        let count_start = lines.iter().take_while(|&(line, _)| line == "\n").count();

        if count_start != 0 {
            for i in count_start..lines.len() {
                lines.swap(i, i - count_start);
            }

            lines.truncate(lines.len() - count_start);
        }

        Some(lines)
    }

    /// Print the base table and all elements inside, including the code in dot form.
    pub fn print_round_info(
        &self,
        options: &[&str],
        code_lines: &[(String, String)],
        width: &usize,
        total_points: u32,
    ) -> Result<()> {
        let pipe = "│".white().dim();

        let points = format!(
            "{padding}{pipe} {}{}\r\n{padding}{pipe} {}{}\r\n{padding}{pipe} {}{}",
            "High Score: ".bold(),
            CONFIG.high_score.to_string().magenta(),
            "Total Points: ".bold(),
            total_points.to_string().cyan(),
            "Available Points: ".bold(),
            Color::RGB(0, 255, 0).paint("100"),
            padding = " ".repeat(7),
        );

        let line_separator_start = "─".repeat(7);
        let line_separator_end = "─".repeat(width - 8);

        let [top, mid, bottom] = ["┬", "┼", "┴"].map(|char| {
            (line_separator_start.clone() + char + &line_separator_end)
                .white()
                .dim()
        });

        let dotted_code = code_lines
            .iter()
            .enumerate()
            .map(|(idx, (line, _))| {
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

        execute!(self.stdout.lock(), Print(text)).map_err(Into::into)
    }

    pub fn get_highlighter(&self, language: &str) -> HighlightLines {
        let syntax = self
            .syntaxes
            .find_syntax_by_name(language)
            .unwrap_or_else(|| self.syntaxes.find_syntax_plain_text());

        HighlightLines::new(syntax, &self.theme)
    }

    /// Create a loop that will reveal a line of code and decrease
    /// `available_points` every 1.5 seconds.
    pub fn start_showing_code(
        &self,
        code_lines: &[(String, String)],
        available_points: &Mutex<f32>,
        receiver: Receiver<()>,
    ) -> Result<()> {
        let mut code_lines: Vec<_> = code_lines.iter().enumerate().collect();

        if ARGS.shuffle {
            code_lines.shuffle(&mut thread_rng());
        };

        // This has to be made a variable as opposed to just checking if idx ==
        // 0 because the lines could be shuffled.
        let mut is_first_line = true;

        // Consume receiver.
        let receiver = receiver;

        for (idx, (raw, line)) in code_lines {
            if raw == "\n" {
                continue;
            }

            let millis = if is_first_line { ARGS.wait } else { 1500 };
            is_first_line = false;

            // The receiver will be notified when the user has selected an
            // option, at which point the code should not be updated further.
            if receiver.recv_timeout(Duration::from_millis(millis)).is_ok() {
                break;
            }

            let mut stdout = self.stdout.lock();

            // Move to the row index of the dotted code and replace it with the
            // real code.
            queue!(stdout, SavePosition, MoveTo(9, idx as u16 + 5), Print(line))?;

            // `available_points` should not be decreased on the first line.
            if idx != 0 {
                let mut available_points = available_points.lock().map_err(|_| "could not lock")?;
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
                )?;
            }

            execute!(stdout, RestorePosition)?;
        }

        Ok(())
    }

    /// Responds to input from the user (1 | 2 | 3 | 4).
    #[allow(clippy::unnecessary_to_owned)]
    pub fn process_input(
        &self,
        num: u32,
        options: &[&str],
        correct_language: &str,
        available_points: &Mutex<f32>,
        total_points: &mut u32,
    ) -> Result<ControlFlow<()>> {
        // Locking the stdout will let any work that's being done in
        // [`Terminal::start_showing_code`] to finish before we continue.
        let mut stdout = self.stdout.lock();

        let correct_option_idx = options
            .iter()
            .position(|&option| option == correct_language)
            .ok_or("correct language not found")?;

        let was_correct = (correct_option_idx + 1) as u32 == num;
        let available_points = available_points.lock().map_err(|_| "could not lock")?;

        let correct_option_name_text = if was_correct {
            format!("{correct_language} (+ {available_points})")
        } else {
            format!("{correct_language} (Correct)")
        };

        let correct_option_text = Self::format_option(
            &(correct_option_idx + 1).to_string(),
            &correct_option_name_text.green().bold().to_string(),
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
    pub fn read_input_char() -> Result<char> {
        // Consume all ready-to-be-collected events to ensure that only future
        // are collected.
        while event::poll(Duration::from_millis(1))? {
            event::read()?;
        }

        loop {
            if let Event::Key(KeyEvent {
                code: KeyCode::Char(char @ ('1' | '2' | '3' | '4' | 'q' | 'c')),
                modifiers,
                ..
            }) = event::read()?
            {
                if char == 'c' && modifiers != KeyModifiers::CONTROL {
                    continue;
                }

                return Ok(char);
            }
        }
    }

    /// Utility function to format an option.
    #[must_use]
    pub fn format_option(key: &str, name: &str) -> String {
        format!(
            "{padding}[{key}] {name}",
            padding = " ".repeat(5),
            key = key.bold(),
            name = name
        )
    }

    /// Get terminal width.
    pub fn width() -> Result<usize> {
        terminal::size()
            .map(|(width, _)| width as usize)
            .map_err(Into::into)
    }
}

#[cfg(target_os = "macos")]
fn macos_dark_mode_active() -> bool {
    let mut defaults_cmd = std::process::Command::new("defaults");
    defaults_cmd.args(&["read", "-globalDomain", "AppleInterfaceStyle"]);
    defaults_cmd
        .output()
        .map_or(true, |output| output.stdout == b"Dark\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref TERMINAL: Terminal = Terminal::new().unwrap();
    }

    const WIDTH: &usize = &500;

    fn parse_code(code: &str) -> Option<Vec<(String, String)>> {
        TERMINAL.parse_code(code, TERMINAL.get_highlighter("Rust"), WIDTH)
    }

    #[test]
    fn cut_off_wide_code() {
        let code = "_".repeat(WIDTH + 1);
        let parsed = parse_code(&code).unwrap();

        assert_eq!(
            parsed[0].0,
            "_".repeat(WIDTH - 3 - "   1   | ".len()) + "..."
        );
    }

    #[test]
    fn remove_comments() {
        let code = indoc! {"
            // Should be removed
            /// Should be removed
            //! Should be removed
            /* Should be removed */
            let x = 5; // Whole line should be removed
            let y = 6;
        "};

        let parsed = parse_code(code).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn cut_off_tall_code() {
        let code = indoc! {"
            Line 1

            Line 2
            Line 3

            Line 4
            Line 5
            Line 6

            Line 7
            Line 8
            Line 9
            Line 10
            Line 11
            Line 12
        "};

        let parsed = parse_code(code).unwrap();
        assert_eq!(parsed.len(), code.lines().count() - 2);
    }

    #[test]
    fn remove_consecutive_duplicate_newlines() {
        let code = indoc! {"
            Line 1


            Line 2


            Line 3
        "};

        let parsed = parse_code(code).unwrap();
        assert_eq!(parsed.len(), code.lines().count() - 2);
    }

    #[test]
    fn trim_newlines() {
        let code = indoc! {"


            Line 1


        "};

        let parsed = parse_code(code).unwrap();
        assert_eq!(parsed.len(), 1);
    }
}
