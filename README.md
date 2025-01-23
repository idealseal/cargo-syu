# Update Rust binary crates

[<img alt="Workflow Status" src="https://img.shields.io/github/actions/workflow/status/idealseal/cargo-syu/.github%2Fworkflows%2Fci.yml?branch=master&logo=GitHub">](https://github.com/idealseal/cargo-syu/actions/workflows/ci.yml)
[<img alt="Crates.io Version" src="https://img.shields.io/crates/v/cargo-syu?logo=rust">](https://crates.io/crates/cargo-syu)

## Installation

```console
cargo install --locked cargo-syu
```

After that, the command can update itself.

## Usage

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
- [x] Add --ask flag to require user confirmation before installing packages.

<sub>

## License

Licensed under the [MIT License](./LICENSE).

</sub>
