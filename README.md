# Update Rust binary crates.

[<img alt="Github" src="https://img.shields.io/badge/github-idealseal/cargo--syu-aaffff?style=flat-square&logo=Github" height="22">](https://github.com/idealseal/cargo-syu)
[<img alt="Workflow Status" src="https://img.shields.io/github/actions/workflow/status/idealseal/cargo-syu/ci.yml?style=for-the-badge" height="22">](https://github.com/idealseal/cargo-syu/actions?query=branch%3Amaster)

## Installation

```console
cargo install --git https://github.com/idealseal/cargo-syu
```

After that, the command can update itself.

## Examples

The default operation is to list and update packages.

```console
cargo syu
```

Don't update packages, but list available updates.

```console
cargo syu --list
```

Take git packages into consideration.

```console
cargo syu --git
```

## Development plan

- [x] Find cargo install root from `~/.cargo/config.toml`.
- [x] List installed packages.
- [x] Update installed packages.
- [x] Find and update git packages.
- [ ] Detect registry URL from `.crates.toml`.
- [ ] Print progress bar for metadata fetching.
- [ ] Improved handling of errors
    1. [ ] Don't fail immediately if one package operation failed.
    2. [ ] Display warning for failed package, but continue for remaining packages.
- [ ] Allow more or less verbose output.
- [ ] Allow printing of outdated packages only.
- [ ] Add more code documentation.
- [ ] Write unit and integration tests.

## License

Licensed under the [MIT License](https://github.com/idealseal/cargo-syu/blob/master/LICENSE).
