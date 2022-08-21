# Guess That Lang!

[![Share Your High Score](https://img.shields.io/badge/-Share%20Your%20High%20Score!-blue?style=for-the-badge&logo=github&logoColor=black)](https://github.com/Lioness100/guess-that-lang/discussions/6)

CLI game to see how fast you can guess the language of a code block!

_If you like
the game, please consider giving a ⭐!_

![Game Demo](https://user-images.githubusercontent.com/65814829/183973036-c283d147-8061-40c8-a306-916801d6c9bc.gif)

Code is retrieved from either random repos or random gists on GitHub
using any of the top 24* most popular languages from the [Stack Overflow 2022
Developer
Survey](https://survey.stackoverflow.co/2022/#most-popular-technologies-language).
<sub>*VBA is replaced with Dockerfile</sub>

The code is then processed in a number of ways to make the
experience more enjoyable.

## Installation

<details>
<summary>Using Prebuilt Binaries</summary>

- <details>
  <summary>With Bash</summary>

  ```sh
  curl -fsSL "https://bina.egoist.dev/Lioness100/guess-that-lang" | sh
  ```

  > Using [Bina](https://bina.egoist.dev/)

  </details>

- <details>
  <summary>Manual Installation</summary>

  Prebuilt binaries are available for Windows, Linux, and macOS and can be found
  attached to the [latest release](https://github.com/Lioness100/guess-that-lang/releases/latest).

  </details>

</details>
<details>
<summary>Building From Source</summary>

> ⚠️ Rust 1.63.0 or higher is required to build. Rust can be updated with `rustup update`.

Install [Rust](https://www.rust-lang.org/tools/install) and then run:

```sh
cargo install guess-that-lang
```

</details>

<br />

## Usage

It's strongly recommended to provide a [Github personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token). This will
allow the game to make more Github requests before getting ratelimited. [Click
here to make
one](https://github.com/settings/tokens/new?description=Guess%20That%20Lang) (no
scopes are required).

> ⚠️ Resizing the terminal window while playing will cause the game to go a bit
> wonky.

```sh
# Tokens will be stored in a config file so you only need to input them once.
guess-that-lang --token "XXX" # or -t

# Get code from gists rather than repos.
# Repos generally provide better code quality, but gists require less API calls.
guess-that-lang --provider gists # or -p

# Wait 5 seconds after showing the options before starting to reveal code. (Default: 1500)
guess-that-lang --wait 5000 # or -w

# Reveal lines in random order instead of top to bottom. (Default: false)
guess-that-lang --shuffle # or -s

# Theme overrides will be stored in a config file so you only need to input them once.
guess-that-lang --theme dark
guess-that-lang --theme light
```

## Acknowledgements

This game takes heavy inspiration from both
[guessthiscode](https://guessthiscode.com) and
[stripcode](https://github.com/benawad/stripcode).

## Contributing

I'm a beginner at Rust, so if you see any code that can be improved or have any
general ideas, please let
me know! Feel free to open an issue or a pull request.
