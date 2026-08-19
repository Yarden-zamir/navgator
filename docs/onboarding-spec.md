# Onboarding Spec

## Goals

- A new user must reach a working setup (config file plus shell shortcuts) from one command.
- Onboarding must explain the binary/wrapper split so users understand why shortcuts go through the wrapper.
- Onboarding must never replace an existing config file without explicit confirmation and a backup, and never modifies existing rc-file content (it only ever appends).
- Onboarding never reads rc files; existing shell setup is detected from environment state only.
- File-write failures degrade to printed instructions; they never abort the walkthrough.
- Onboarding is a plain line-based terminal dialog, not a TUI screen.

## Command

- `navgator onboarding` and the alias `navgator --onboarding` run the walkthrough.
- Onboarding rejects `--config-entry`.
- Onboarding requires an interactive terminal; stdin is re-opened from the tty like other interactive commands.
- A closed stdin fails onboarding with a clear error instead of looping.

## Intro

- Onboarding starts with a short summary: the binary prints the picked path; the zsh wrapper widgets (`navigate`, `navgator-create`, `navgator-create-new-project`) run the binary and perform the `cd`.
- Onboarding states the Nerd Font requirement with a link.
- Onboarding prints full paths without collapsing the home directory, so runs with an alternate `HOME` or `NAVGATOR_CONFIG` show the real location.

## Config Step

- Config discovery uses the same order as normal config loading, including the `NAVGATOR_CONFIG` single-file override.
- When `NAVGATOR_CONFIG` is set to a non-empty value, onboarding says so and uses only that path.
- When an existing config file is found, onboarding reports its path and asks whether to keep it; keeping is the default.
- Choosing to replace prompts for index folders, renames the existing file to the first free `.bak` name (`config.toml.bak`, then `.bak.1`, `.bak.2`, ...), and writes a fresh starter config at the same path.
- Earlier backups are never overwritten.
- A failed backup rename keeps the existing config untouched and reports the error instead of replacing.
- The backup location is reported before the new config is written.
- When writing the new config fails after the backup rename, the backup is renamed back; if even that fails, the backup path is reported for manual recovery.
- When creating a fresh starter config fails, onboarding reports the error, notes that navgator runs with built-in defaults, and continues to the shell step.
- When no config file exists, onboarding prompts for comma-separated index folders with `~/Github, ~/Projects` as the default.
- An empty answer keeps the default index folders.
- Folders that do not exist yet are accepted with a note that they are indexed once created.
- Onboarding writes the starter config to the default user config path with the chosen index folders; all other content matches written defaults from the config behavior spec.
- Onboarding reports the created path and the indexed folders.

## Shell Step

- Onboarding looks for `navgator.zsh` relative to the running binary: the version-independent `<brew root>/opt/navgator/share/navgator/navgator.zsh` first for Homebrew Cellar installs, then `<prefix>/share/navgator/navgator.zsh`, then `<repo>/scripts/navgator.zsh` for the cargo `target/{release,debug}` layout.
- When the wrapper script is not found, onboarding prints the manual Homebrew setup lines and skips the shell step without failing.
- When `$SHELL` does not contain `zsh`, onboarding notes that the wrapper currently supports zsh only and continues.
- The target rc file is `$ZDOTDIR/.zshrc` when `ZDOTDIR` is set and non-empty, otherwise `~/.zshrc`; it is appended to, never read.
- The sourced wrapper exports `NAVGATOR_ZSH_SOURCED` with its resolved path; onboarding uses that variable as the only signal that the wrapper is active in the current shell.
- When `NAVGATOR_ZSH_SOURCED` is set, onboarding asks whether to add shortcut bindings anyway (default no); accepting appends a block without a `source` line.
- Known limit: re-running onboarding from a shell that has not sourced the freshly appended setup appends a duplicate block, because rc files are never read.

## Shortcut Capture

- Shortcuts are captured as a live keypress in raw mode: `navigate` defaults to `ctrl+t`, `navgator-create-new-project` defaults to `ctrl+n`.
- `Enter` accepts the default; `Esc` switches to typed entry; a terminal without raw capture falls back to typed entry automatically.
- A captured chord is echoed and must be confirmed before it is used; declining re-captures.
- Valid chords combine ctrl and/or alt with a letter (alt also allows digits), or are a bare function key `f1` through `f12`; a character key without ctrl or alt is rejected with the reason.
- Function keys with ctrl or alt are rejected as not portably bindable.
- Ctrl-only chords on `c`, `d`, `z`, `i`, `m`, `j`, `q`, and `s` are rejected as terminal-reserved, with the reason.
- `ctrl+c` during capture cancels onboarding.
- The second shortcut cannot repeat the first.
- Typed entry accepts forms like `ctrl+t`, `alt+g`, `ctrl+alt+p`, and `f5`; a bare letter means ctrl+letter (bare digits get no implied modifier); an empty answer keeps the default.
- Bindkey sequences are `^T` for ctrl, `^[t` for alt, and `^[^T` for ctrl+alt; function keys bind through `"${terminfo[kfN]}"` so the sequence matches the user's terminal.
- On macOS, choosing an alt chord prints a note that the terminal must send option as Esc+/meta.

## Append

- Onboarding shows the exact block before touching the rc file and asks for confirmation, defaulting to yes.
- The appended block contains a marker comment, a `source` line with the resolved wrapper path (omitted when the wrapper is already sourced), and one `bindkey` line per shortcut.
- Declining the append prints the block for manual use instead of writing.
- A failed rc-file write reports the error and prints the lines for manual use instead of failing onboarding.
- The success message names the chosen shortcut for each widget.

## Summary

- The summary reflects what actually happened: the config path (or that navgator runs with built-in defaults when nothing was written), and the chosen navigate shortcut only when bindings were appended; otherwise it says shell shortcuts are unchanged.

## Verification

- Onboarding changes must run `cargo fmt -- --check`.
- Onboarding changes must run `cargo clippy --all-targets --all-features -- -D warnings`.
- Onboarding changes must run `cargo test`.
