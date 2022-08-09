# Guess That Lang!

CLI game to see how fast you can guess the language of a code block!

_If you like
the game, please consider giving a ⭐!_

![Game Demo](https://user-images.githubusercontent.com/65814829/183648642-28ceaa6c-8e90-4375-9aa4-0d66fe3e9226.gif)

Code is retrieved from [random
gists](https://docs.github.com/en/rest/gists/gists#list-public-gists) on GitHub
using any of the top 24* most popular languages from the [Stack Overflow 2022
Developer
Survey](https://survey.stackoverflow.co/2022/#most-popular-technologies-language).
<sub>*VBA is replaced with Dockerfile</sub>

## Installation

Install [Rust](https://www.rust-lang.org/tools/install) using the recommended rustup installation method and then run:

```sh
cargo install guess-that-lang
```

## Usage

It's strongly recommended to provide a [Github personal access token](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token). This will
allow the game to make more Github requests before getting ratelimited. [Click
here to make
one](https://github.com/settings/tokens/new?description=Guess%20That%20Lang) (no
scopes are required).

> ⚠️ Resizing the terminal window while playing will cause the game to go a bit
> wonky.

```sh
# Tokens will be stored in .guess-that-lang and thus are only need to be input once.
guess-that-lang --token "XXX"
guess-that-lang
```

## Acknowledgements

This game takes heavy inspiration from both
[guessthiscode](https://guessthiscode.com) and
[stripcode](https://github.com/benawad/stripcode).

## Contributing

I'm a beginner at Rust, so if you see any code that can be improved or have any
general ideas, please let
me know! Feel free to open an issue or a pull request.
