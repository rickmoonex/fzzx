# fzzx

`fzzx` is a small, scriptable fuzzy picker for macOS. It reads newline-separated
choices from stdin, opens a native AppKit menu, and writes the selected choice to
stdout. The UI is deliberately dmenu-like: one square, borderless rectangle with
no title bar, rounded launcher card, shadow, animation, or decorative effects.
Installed Nerd Fonts work like any other macOS font.

```sh
printf '󰀻 Safari\n󰈹 Firefox\n󰒓 System Settings\n' |
  fzzx --font 'JetBrainsMono Nerd Font Mono:size=16'
```

Use it in scripts just like a dmenu-style Fuzzel invocation:

```sh
choice=$(some-command | fzzx --dmenu --prompt 'Run: ')
[ -n "$choice" ] && open -a "$choice"
```

With no stdin, `fzzx` becomes a prompt and prints the entered text:

```sh
note=$(fzzx --prompt 'Note: ')
```

`fzzx` exits with `0` after a selection or submitted prompt, `1` on
cancellation, and `2` for invalid arguments, configuration, or input. Output is
terminated by a newline. `--index` outputs the selected row's original
zero-based index and requires stdin choices.

## Install

Build, run, or install directly with Nix:

```sh
nix build
nix run . -- --help
nix profile install .#
```

Or install from this repository as a Homebrew tap:

```sh
brew tap rickmoonex/fzzx https://github.com/rickmoonex/fzzx
brew install rickmoonex/fzzx/fzzx
```

Upgrade later with `brew upgrade rickmoonex/fzzx/fzzx`. The explicit URL is
needed because this repository is named `fzzx`, not `homebrew-fzzx`.

Each GitHub release also contains unsigned archives for both macOS
architectures:

- `aarch64-apple-darwin` for Apple Silicon
- `x86_64-apple-darwin` for Intel Macs

After downloading the matching archive and its `.sha256` file from the release
page:

```sh
VERSION=0.1.0
TARGET=aarch64-apple-darwin
shasum -a 256 -c "fzzx-v${VERSION}-${TARGET}.sha256"
tar -xzf "fzzx-v${VERSION}-${TARGET}.tar.gz"
install -m 755 fzzx "$HOME/.local/bin/fzzx"
```

Make sure `$HOME/.local/bin` is on `PATH`. The archives also contain the README,
license, and changelog.

The flake supports `aarch64-darwin` and `x86_64-darwin`. To consume it from
another flake:

```nix
{
  inputs.fzzx.url = "github:OWNER/fzzx";
}
```

Then, in a nix-darwin module:

```nix
{ inputs, pkgs, ... }:
{
  environment.systemPackages = [
    inputs.fzzx.packages.${pkgs.stdenv.hostPlatform.system}.default
  ];
}
```

Or replace `environment.systemPackages` with `home.packages` in Home Manager.

For local development:

```sh
nix develop
cargo test
cargo run -- --help
```

## Configuration

The default configuration path is `$XDG_CONFIG_HOME/fzzx/fzzx.ini`, falling
back to `~/.config/fzzx/fzzx.ini`. CLI options override the file.

```ini
[main]
# Installed font family or PostScript name. Add :size=N for a size from 6 to 96.
# Omit this key to use the 16-point macOS monospaced system font.
font=JetBrainsMono Nerd Font Mono:size=16

# Text shown in the block to the left of the input. May be empty. Default: >
prompt=Choose:

# Maximum visible result rows, from 1 to 8. Short lists use only the rows needed.
# Lists longer than this scroll as the selection moves. Default: 8
lines=8

# Panel width in macOS points. Must be at least 200. Default: 640
width=640

[colors]
# Colors are RRGGBB or RRGGBBAA; a leading # is optional.

# Panel and input background. Default: 1f1b17f5
background=1f1b17f5

# Normal result and input text. Default: d6d1c9ff
text=d6d1c9ff

# Prompt text. Default: 1f1b17ff
prompt=1f1b17ff

# Prompt block background. Default: a39c94ff
prompt-background=a39c94ff

# Selected result background. Default: bdb5adff
selection=bdb5adff

# Selected result text. Default: 1f1b17ff
selection-text=1f1b17ff

# Fuzzy-matched characters in unselected results. Default: f5bd6bff
match=f5bd6bff
```

This is the complete configuration schema; unknown sections or keys are errors
so typos do not silently change behavior. Without color settings, the built-in
high-contrast dark dmenu palette shown in the comments is used.

Every config value has a CLI override. The mappings are `prompt` to `--prompt`,
`lines` to `--lines`, `width` to `--width`, `font` to `--font`, and each color
name to its `--*-color` equivalent. The two exceptions are `background`, which
uses `--background`, and `prompt-background`, which uses
`--prompt-background`. CLI values override only the corresponding config value,
for example:

```sh
printf 'Alpha\nBeta\nGamma\n' |
  fzzx --width 900 --selection-color 87af87 --match-color ff5f5f
```

The panel shows only as many rows as it has initial matches, up to `lines`. The
maximum is 8; longer lists scroll as the keyboard selection moves.

Run `fzzx --help` for the complete CLI.

## Releases

Pull requests run formatting, tests, Clippy, Nix evaluation, and a Nix package
build. On `main`, release-plz maintains a release PR from conventional commits.
Merging that PR bumps `Cargo.toml` and `Cargo.lock`, updates `CHANGELOG.md`, tags
the commit, publishes the changelog as a GitHub release, and attaches native
Apple Silicon and Intel archives with SHA-256 checksums. This project is not
published to crates.io because release-plz is configured for git-only releases.
After both archives are uploaded, the workflow updates `Formula/fzzx.rb` on
`main` with the released version and checksums.

The release job checks every push to `main`, but git-only version detection
creates a release only when `Cargo.toml` is newer than the latest `v*` tag. The
release-PR job runs after that check, avoiding races during the first release.

Use conventional commit prefixes such as `fix:`, `feat:`, and `feat!:` so
release-plz can determine patch, minor, and major version bumps.

Enable the repository's dependency-free Conventional Commit hook once after
cloning:

```sh
git config core.hooksPath .githooks
```

It validates normal commits while allowing Git-generated merge, revert, fixup,
and squash messages.

The repository's Actions settings must allow GitHub Actions to create pull
requests and push to `main` so the tap formula can be refreshed. Release-plz
uses the built-in `GITHUB_TOKEN`; no crates.io token is needed. The formula
becomes installable after the first GitHub release populates its real checksums.

## Keys

- Type to filter.
- Up, Down, Control-P, and Control-N move through results.
- Page Up and Page Down move by one visible page.
- Return selects; Escape cancels.
- Standard macOS editing shortcuts work in the query field.
