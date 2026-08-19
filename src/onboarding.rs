use std::{
    env, fs,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::config::{
    config_file_paths, default_user_config_path, starter_config_contents, DEFAULT_INDEX_FOLDERS,
};
use crate::model::AppResult;

const NAVIGATE_WIDGET: &str = "navigate";
const CREATE_WIDGET: &str = "navgator-create-new-project";

pub(crate) fn run_onboarding() -> AppResult<()> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let home = gator::config::home_dir()?;

    print_intro();
    let config_path = onboard_config(&mut input, &home)?;
    let shortcuts = onboard_shell(&mut input, &home)?;
    print_summary(config_path.as_deref(), shortcuts);
    Ok(())
}

fn print_intro() {
    println!("🐊 navgator onboarding\n");
    println!("Two pieces work together:");
    println!("  • `navgator` — the TUI binary; it prints the path you pick");
    println!("  • `navgator.zsh` — the zsh wrapper defining the `navigate`,");
    println!("    `navgator-create`, and `navgator-create-new-project` widgets, which run");
    println!("    the binary and `cd` your shell to the selection");
    println!("Bind shortcuts to the wrapper widgets; use the binary for everything else.\n");
    println!("navgator needs a Nerd Font (https://www.nerdfonts.com) to render its icons.\n");
}

fn onboard_config(input: &mut impl BufRead, home: &Path) -> AppResult<Option<PathBuf>> {
    println!("── Step 1 of 2: config file ──\n");
    if let Ok(value) = env::var("NAVGATOR_CONFIG") {
        if !value.trim().is_empty() {
            println!("NAVGATOR_CONFIG is set, so only that path is used: {value}");
        }
    }
    if let Some(existing) = config_file_paths(home).into_iter().find(|p| p.is_file()) {
        println!("Found an existing config: {}", existing.display());
        if confirm(
            input,
            "Keep it? [Y/n] (n = replace with a fresh starter config, keeping a .bak): ",
            true,
        )? {
            println!("Keeping it untouched.\n");
            return Ok(Some(existing));
        }
        let folders = ask_index_folders(input, home)?;
        let backup = backup_path(&existing);
        if let Err(error) = fs::rename(&existing, &backup) {
            println!("Could not back up {}: {error}", existing.display());
            println!("Keeping the existing config untouched.\n");
            return Ok(Some(existing));
        }
        println!("Backed up the old config to {}.", backup.display());
        if let Err(error) = write_starter_config(&existing, &folders) {
            if fs::rename(&backup, &existing).is_ok() {
                println!("Could not write the new config: {error}");
                println!("Restored the original config.\n");
                return Ok(Some(existing));
            }
            println!("Could not write the new config: {error}");
            println!(
                "Your old config is at {}; restore it manually.\n",
                backup.display()
            );
            return Ok(None);
        }
        println!(
            "\nCreated {} indexing: {}\n",
            existing.display(),
            folders.join(", ")
        );
        return Ok(Some(existing));
    }

    let target = default_user_config_path(home);
    println!(
        "No config file found. A starter config will be created at {}",
        target.display()
    );
    println!("with every built-in action, create recipe, and keybinding written out.\n");
    let folders = ask_index_folders(input, home)?;
    if let Err(error) = write_starter_config(&target, &folders) {
        println!("Could not write {}: {error}", target.display());
        println!("navgator still runs with built-in defaults; re-run onboarding later.\n");
        return Ok(None);
    }
    println!(
        "\nCreated {} indexing: {}\n",
        target.display(),
        folders.join(", ")
    );
    Ok(Some(target))
}

fn ask_index_folders(input: &mut impl BufRead, home: &Path) -> AppResult<Vec<String>> {
    let default_folders = DEFAULT_INDEX_FOLDERS.join(", ");
    let answer = prompt(
        input,
        &format!("Folders holding your projects, comma-separated [{default_folders}]: "),
    )?;
    let mut folders = parse_index_folders(&answer);
    if folders.is_empty() {
        folders = DEFAULT_INDEX_FOLDERS
            .iter()
            .map(|folder| folder.to_string())
            .collect();
    }
    for missing in missing_folders(&folders, home) {
        println!("Note: {missing} does not exist yet; it is indexed once created.");
    }
    Ok(folders)
}

fn write_starter_config(target: &Path, folders: &[String]) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, starter_config_contents(folders))
}

/// First free backup name: `config.toml.bak`, then `config.toml.bak.1`, ...
/// so an earlier backup is never overwritten.
fn backup_path(existing: &Path) -> PathBuf {
    let base = PathBuf::from(format!("{}.bak", existing.display()));
    if !base.exists() {
        return base;
    }
    let mut counter = 1u32;
    loop {
        let candidate = PathBuf::from(format!("{}.bak.{counter}", existing.display()));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn onboard_shell(input: &mut impl BufRead, home: &Path) -> AppResult<Option<(Chord, Chord)>> {
    println!("── Step 2 of 2: shell shortcuts ──\n");
    if let Ok(shell) = env::var("SHELL") {
        if !shell.contains("zsh") {
            println!("Note: your $SHELL is {shell}; the wrapper currently supports zsh only.");
        }
    }

    // Setup detection is shell-state only: the wrapper exports
    // NAVGATOR_ZSH_SOURCED when sourced, and the rc file is never read.
    // Limit: re-running onboarding from a shell that has not sourced the
    // freshly appended setup yet appends a duplicate block. Revisit if
    // duplicate blocks show up in practice.
    let sourced = env::var("NAVGATOR_ZSH_SOURCED")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let source_wrapper = if let Some(path) = &sourced {
        println!("The wrapper is already sourced in this shell: {path}");
        if !confirm(input, "Add shortcut bindings anyway? [y/N]: ", false)? {
            println!("Leaving shell setup as is.\n");
            return Ok(None);
        }
        None
    } else {
        match wrapper_script_path() {
            Some(wrapper) => {
                println!("The wrapper was found at {}.", wrapper.display());
                Some(wrapper)
            }
            None => {
                println!("Could not locate `navgator.zsh` next to this binary.");
                println!("If you installed with Homebrew, add this to your ~/.zshrc yourself:");
                println!("  source \"$(brew --prefix navgator)/share/navgator/navgator.zsh\"");
                println!("  bindkey '^T' {NAVIGATE_WIDGET}");
                println!("  bindkey '^N' {CREATE_WIDGET}\n");
                return Ok(None);
            }
        }
    };

    println!("Shortcuts: ctrl/alt plus a letter or digit, or a function key (f1-f12).\n");
    let navigate_chord = ask_chord(input, NAVIGATE_WIDGET, Chord::ctrl('t'), &[])?;
    let create_chord = ask_chord(input, CREATE_WIDGET, Chord::ctrl('n'), &[navigate_chord])?;
    warn_alt_on_macos(&[navigate_chord, create_chord]);

    let zshrc = zshrc_path(home);
    let zshrc_display = zshrc.display();
    let block = zshrc_block(source_wrapper.as_deref(), navigate_chord, create_chord);
    println!("\nThis will be appended to {zshrc_display}:\n{block}");
    if confirm(input, "Append it now? [Y/n]: ", true)? {
        match append_to_file(&zshrc, &block) {
            Ok(()) => {
                println!("Added. Restart your shell or run `source {zshrc_display}`, then");
                println!(
                    "press {} to navigate and {} to create a project.\n",
                    navigate_chord.label(),
                    create_chord.label()
                );
                Ok(Some((navigate_chord, create_chord)))
            }
            Err(error) => {
                println!("Could not write {zshrc_display}: {error}");
                println!("Add the lines above yourself.\n");
                Ok(None)
            }
        }
    } else {
        println!("Skipped. Add the lines above yourself when ready.\n");
        Ok(None)
    }
}

fn warn_alt_on_macos(chords: &[Chord]) {
    if env::consts::OS == "macos" && chords.iter().any(|chord| chord.alt) {
        println!("\nNote: alt shortcuts need your terminal to send option as Esc+/meta");
        println!("(Terminal.app: Settings → Profiles → Keyboard; iTerm2: option key → Esc+).");
    }
}

fn print_summary(config_path: Option<&Path>, shortcuts: Option<(Chord, Chord)>) {
    println!("── Done ──\n");
    match config_path {
        Some(path) => println!("Config: {} (edit any time)", path.display()),
        None => println!("No config file was written; navgator runs with built-in defaults."),
    }
    match shortcuts {
        Some((navigate, _)) => println!(
            "Open a new shell (or source your zshrc), then press {} to navigate.",
            navigate.label()
        ),
        None => println!("Run `navgator` directly to explore; shell shortcuts are unchanged."),
    }
    println!("`navgator config-schema` prints the full config schema.");
}

fn prompt(input: &mut impl BufRead, message: &str) -> AppResult<String> {
    print!("{message}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Err("onboarding needs an interactive terminal (stdin closed)".into());
    }
    Ok(line.trim().to_string())
}

fn confirm(input: &mut impl BufRead, message: &str, default_yes: bool) -> AppResult<bool> {
    let answer = prompt(input, message)?.to_ascii_lowercase();
    if answer.is_empty() {
        return Ok(default_yes);
    }
    Ok(answer == "y" || answer == "yes")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ChordKey {
    Char(char),
    Function(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Chord {
    ctrl: bool,
    alt: bool,
    key: ChordKey,
}

impl Chord {
    fn ctrl(key: char) -> Self {
        Self {
            ctrl: true,
            alt: false,
            key: ChordKey::Char(key),
        }
    }

    fn label(&self) -> String {
        let mut label = String::new();
        if self.ctrl {
            label.push_str("ctrl+");
        }
        if self.alt {
            label.push_str("alt+");
        }
        match self.key {
            ChordKey::Char(character) => label.push(character),
            ChordKey::Function(number) => label.push_str(&format!("f{number}")),
        }
        label
    }

    /// One zsh bindkey line. Char chords use escape notation (`^T` for ctrl,
    /// `^[t` for alt, `^[^T` for both); function keys bind through terminfo so
    /// the sequence matches the user's terminal.
    fn bindkey_line(&self, widget: &str) -> String {
        match self.key {
            ChordKey::Char(character) => {
                let mut sequence = String::new();
                if self.alt {
                    sequence.push_str("^[");
                }
                if self.ctrl {
                    sequence.push('^');
                    sequence.push(character.to_ascii_uppercase());
                } else {
                    sequence.push(character);
                }
                format!("bindkey '{sequence}' {widget}")
            }
            ChordKey::Function(number) => {
                format!("bindkey \"${{terminfo[kf{number}]}}\" {widget}")
            }
        }
    }
}

/// Ctrl chords the terminal or shell already claims; binding them would break
/// interrupts, EOF, suspend, Tab, Enter, or flow control.
const RESERVED_CTRL_KEYS: [char; 8] = ['c', 'd', 'z', 'i', 'm', 'j', 'q', 's'];

fn validate_chord(ctrl: bool, alt: bool, key: ChordKey, taken: &[Chord]) -> Result<Chord, String> {
    let chord = match key {
        ChordKey::Function(number) => {
            if !(1..=12).contains(&number) {
                return Err("only f1 through f12 are supported".to_string());
            }
            if ctrl || alt {
                return Err(
                    "modified function keys are not portably bindable; use the bare function key"
                        .to_string(),
                );
            }
            Chord {
                ctrl: false,
                alt: false,
                key,
            }
        }
        ChordKey::Char(character) => {
            let character = character.to_ascii_lowercase();
            if !character.is_ascii_alphanumeric() {
                return Err("use a letter, digit, or function key".to_string());
            }
            if !ctrl && !alt {
                return Err(
                    "include ctrl or alt, otherwise typing that key would trigger it".to_string(),
                );
            }
            if ctrl && !alt && RESERVED_CTRL_KEYS.contains(&character) {
                return Err(format!(
                    "ctrl+{character} is reserved by the terminal (interrupt/EOF/suspend/tab/enter/flow control)"
                ));
            }
            if ctrl && !character.is_ascii_alphabetic() {
                return Err("ctrl chords only work with letters".to_string());
            }
            Chord {
                ctrl,
                alt,
                key: ChordKey::Char(character),
            }
        }
    };
    if taken.contains(&chord) {
        return Err(format!(
            "{} is already used by the previous shortcut",
            chord.label()
        ));
    }
    Ok(chord)
}

enum Capture {
    Key {
        ctrl: bool,
        alt: bool,
        key: ChordKey,
    },
    Enter,
    Esc,
    Cancelled,
    Other,
    Unsupported,
}

struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Option<Self> {
        enable_raw_mode().ok().map(|()| Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn capture_key() -> Capture {
    let Some(_guard) = RawModeGuard::enable() else {
        return Capture::Unsupported;
    };
    loop {
        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                return match key.code {
                    KeyCode::Enter if !ctrl && !alt => Capture::Enter,
                    KeyCode::Esc => Capture::Esc,
                    KeyCode::Char('c') if ctrl && !alt => Capture::Cancelled,
                    KeyCode::Char(character) => Capture::Key {
                        ctrl,
                        alt,
                        key: ChordKey::Char(character),
                    },
                    KeyCode::F(number) => Capture::Key {
                        ctrl,
                        alt,
                        key: ChordKey::Function(number),
                    },
                    _ => Capture::Other,
                };
            }
            Ok(_) => continue,
            Err(_) => return Capture::Unsupported,
        }
    }
}

fn ask_chord(
    input: &mut impl BufRead,
    widget: &str,
    default: Chord,
    taken: &[Chord],
) -> AppResult<Chord> {
    loop {
        print!(
            "Press the shortcut for `{widget}` (Enter = {}, Esc = type it instead): ",
            default.label()
        );
        std::io::stdout().flush()?;
        let capture = capture_key();
        match capture {
            Capture::Enter => {
                println!("{}", default.label());
                if let Err(reason) = validate_chord(default.ctrl, default.alt, default.key, taken) {
                    println!("The default cannot be used here: {reason}.");
                    continue;
                }
                return Ok(default);
            }
            Capture::Esc | Capture::Unsupported => {
                if matches!(capture, Capture::Unsupported) {
                    println!("(key capture is not supported in this terminal)");
                } else {
                    println!("typing");
                }
                return ask_chord_typed(input, widget, default, taken);
            }
            Capture::Cancelled => {
                println!("^C");
                return Err("onboarding cancelled".into());
            }
            Capture::Other => {
                println!("?");
                println!(
                    "That key cannot be bound; use ctrl/alt plus a letter or digit, or f1-f12."
                );
            }
            Capture::Key { ctrl, alt, key } => match validate_chord(ctrl, alt, key, taken) {
                Ok(chord) => {
                    println!("{}", chord.label());
                    if confirm(
                        input,
                        &format!("Use {} for `{widget}`? [Y/n]: ", chord.label()),
                        true,
                    )? {
                        return Ok(chord);
                    }
                }
                Err(reason) => {
                    println!("{}", Chord { ctrl, alt, key }.label());
                    println!("Cannot use that: {reason}.");
                }
            },
        }
    }
}

fn ask_chord_typed(
    input: &mut impl BufRead,
    widget: &str,
    default: Chord,
    taken: &[Chord],
) -> AppResult<Chord> {
    loop {
        let answer = prompt(
            input,
            &format!(
                "Shortcut for `{widget}` like ctrl+t, alt+g, or f5 [{}]: ",
                default.label()
            ),
        )?;
        let Some((ctrl, alt, key)) = parse_chord_text(&answer, default) else {
            println!("Enter a chord like `ctrl+t`, `alt+g`, `ctrl+alt+p`, or `f5`.");
            continue;
        };
        match validate_chord(ctrl, alt, key, taken) {
            Ok(chord) => return Ok(chord),
            Err(reason) => println!("Cannot use that: {reason}."),
        }
    }
}

fn parse_chord_text(value: &str, default: Chord) -> Option<(bool, bool, ChordKey)> {
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Some((default.ctrl, default.alt, default.key));
    }
    let mut ctrl = false;
    let mut alt = false;
    let mut key = None;
    let mut parts = trimmed.split('+').peekable();
    while let Some(part) = parts.next() {
        let last = parts.peek().is_none();
        match part.trim() {
            "ctrl" | "control" if !last => ctrl = true,
            "alt" | "meta" | "opt" | "option" if !last => alt = true,
            part if last => key = parse_key_name(part),
            _ => return None,
        }
    }
    let key = key?;
    // A bare letter means ctrl+letter, matching the suggested defaults.
    if !ctrl && !alt {
        if let ChordKey::Char(character) = key {
            if character.is_ascii_alphabetic() {
                ctrl = true;
            }
        }
    }
    Some((ctrl, alt, key))
}

fn parse_key_name(part: &str) -> Option<ChordKey> {
    if let Some(number) = part.strip_prefix('f') {
        if !number.is_empty() {
            return number.parse::<u8>().ok().map(ChordKey::Function);
        }
    }
    let mut chars = part.chars();
    match (chars.next(), chars.next()) {
        (Some(character), None) => Some(ChordKey::Char(character)),
        _ => None,
    }
}

fn parse_index_folders(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|folder| !folder.is_empty())
        .map(str::to_string)
        .collect()
}

fn missing_folders(folders: &[String], home: &Path) -> Vec<String> {
    folders
        .iter()
        .filter(|folder| {
            gator::config::normalize_configured_path(folder, home, home)
                .map(|path| !path.is_dir())
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn zshrc_block(wrapper: Option<&Path>, navigate: Chord, create: Chord) -> String {
    let mut block = String::from("\n# navgator (added by `navgator onboarding`)\n");
    if let Some(wrapper) = wrapper {
        block.push_str(&format!("source \"{}\"\n", wrapper.display()));
    }
    block.push_str(&navigate.bindkey_line(NAVIGATE_WIDGET));
    block.push('\n');
    block.push_str(&create.bindkey_line(CREATE_WIDGET));
    block.push('\n');
    block
}

fn append_to_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

fn wrapper_script_path() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    wrapper_script_candidates(&exe)
        .into_iter()
        .find(|path| path.is_file())
}

fn wrapper_script_candidates(exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    // Homebrew: prefer the version-independent opt path so the source line
    // survives upgrades: <brew root>/Cellar/navgator/<v>/bin/navgator →
    // <brew root>/opt/navgator/share/navgator/navgator.zsh
    for ancestor in exe.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "Cellar") {
            if let Some(brew_root) = ancestor.parent() {
                candidates.push(brew_root.join("opt/navgator/share/navgator/navgator.zsh"));
            }
        }
    }
    // Generic prefix layout: <prefix>/bin/navgator → <prefix>/share/navgator/navgator.zsh
    if let Some(prefix) = exe.parent().and_then(Path::parent) {
        candidates.push(prefix.join("share/navgator/navgator.zsh"));
    }
    // Cargo layout: <repo>/target/{release,debug}/navgator → <repo>/scripts/navgator.zsh
    if let Some(repo) = exe.parent().and_then(Path::parent).and_then(Path::parent) {
        candidates.push(repo.join("scripts/navgator.zsh"));
    }
    candidates
}

fn zshrc_path(home: &Path) -> PathBuf {
    match env::var("ZDOTDIR") {
        Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir).join(".zshrc"),
        _ => home.join(".zshrc"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_separated_index_folders() {
        assert_eq!(
            parse_index_folders(" ~/Github , ~/Work ,, "),
            vec!["~/Github".to_string(), "~/Work".to_string()]
        );
        assert!(parse_index_folders("   ").is_empty());
    }

    #[test]
    fn backup_path_never_overwrites_previous_backups() {
        let dir = std::env::temp_dir().join(format!(
            "navgator-backup-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let config = dir.join("config.toml");
        let first = backup_path(&config);
        assert_eq!(first, dir.join("config.toml.bak"));
        fs::write(&first, "old").expect("write first backup");
        let second = backup_path(&config);
        assert_eq!(second, dir.join("config.toml.bak.1"));
        fs::write(&second, "older").expect("write second backup");
        assert_eq!(backup_path(&config), dir.join("config.toml.bak.2"));
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn parses_typed_chords() {
        let default = Chord::ctrl('t');
        assert_eq!(
            parse_chord_text("", default),
            Some((true, false, ChordKey::Char('t')))
        );
        assert_eq!(
            parse_chord_text("g", default),
            Some((true, false, ChordKey::Char('g')))
        );
        assert_eq!(
            parse_chord_text("ctrl+n", default),
            Some((true, false, ChordKey::Char('n')))
        );
        assert_eq!(
            parse_chord_text("alt+G", default),
            Some((false, true, ChordKey::Char('g')))
        );
        assert_eq!(
            parse_chord_text("ctrl+alt+p", default),
            Some((true, true, ChordKey::Char('p')))
        );
        assert_eq!(
            parse_chord_text("f5", default),
            Some((false, false, ChordKey::Function(5))),
            "bare function key needs no implied ctrl"
        );
        assert_eq!(
            parse_chord_text("F12", default),
            Some((false, false, ChordKey::Function(12)))
        );
        assert_eq!(
            parse_chord_text("ctrl+f5", default),
            Some((true, false, ChordKey::Function(5)))
        );
        assert_eq!(
            parse_chord_text("1", default),
            Some((false, false, ChordKey::Char('1'))),
            "bare digit gets no implied ctrl"
        );
        assert_eq!(parse_chord_text("ctrl+", default), None);
        assert_eq!(parse_chord_text("nope", default), None);
        assert_eq!(parse_chord_text("ctrl", default), None);
    }

    #[test]
    fn validates_chords() {
        assert!(validate_chord(true, false, ChordKey::Char('T'), &[])
            .is_ok_and(|chord| chord.key == ChordKey::Char('t')));
        assert!(validate_chord(false, true, ChordKey::Char('1'), &[]).is_ok());
        assert!(
            validate_chord(false, false, ChordKey::Char('g'), &[]).is_err(),
            "modifier required"
        );
        assert!(
            validate_chord(true, false, ChordKey::Char('c'), &[]).is_err(),
            "ctrl+c reserved"
        );
        assert!(
            validate_chord(true, true, ChordKey::Char('c'), &[]).is_ok(),
            "ctrl+alt+c allowed"
        );
        assert!(
            validate_chord(true, false, ChordKey::Char('1'), &[]).is_err(),
            "ctrl+digit rejected"
        );
        assert!(
            validate_chord(true, false, ChordKey::Char('.'), &[]).is_err(),
            "punctuation rejected"
        );
        assert!(
            validate_chord(false, false, ChordKey::Function(5), &[]).is_ok(),
            "bare function key allowed"
        );
        assert!(
            validate_chord(true, false, ChordKey::Function(5), &[]).is_err(),
            "modified function key rejected"
        );
        assert!(
            validate_chord(false, false, ChordKey::Function(13), &[]).is_err(),
            "f13 rejected"
        );
        assert!(
            validate_chord(true, false, ChordKey::Char('t'), &[Chord::ctrl('t')]).is_err(),
            "duplicate rejected"
        );
    }

    #[test]
    fn chords_render_bindkey_lines_and_labels() {
        assert_eq!(
            Chord::ctrl('t').bindkey_line("navigate"),
            "bindkey '^T' navigate"
        );
        assert_eq!(Chord::ctrl('t').label(), "ctrl+t");
        let alt_g = Chord {
            ctrl: false,
            alt: true,
            key: ChordKey::Char('g'),
        };
        assert_eq!(alt_g.bindkey_line("navigate"), "bindkey '^[g' navigate");
        assert_eq!(alt_g.label(), "alt+g");
        let both = Chord {
            ctrl: true,
            alt: true,
            key: ChordKey::Char('p'),
        };
        assert_eq!(both.bindkey_line("navigate"), "bindkey '^[^P' navigate");
        assert_eq!(both.label(), "ctrl+alt+p");
        let f5 = Chord {
            ctrl: false,
            alt: false,
            key: ChordKey::Function(5),
        };
        assert_eq!(
            f5.bindkey_line("navigate"),
            "bindkey \"${terminfo[kf5]}\" navigate"
        );
        assert_eq!(f5.label(), "f5");
    }

    #[test]
    fn zshrc_block_sources_wrapper_and_binds_widgets() {
        let block = zshrc_block(
            Some(Path::new("/opt/navgator/navgator.zsh")),
            Chord::ctrl('t'),
            Chord::ctrl('n'),
        );
        assert!(block.contains("source \"/opt/navgator/navgator.zsh\""));
        assert!(block.contains("bindkey '^T' navigate"));
        assert!(block.contains("bindkey '^N' navgator-create-new-project"));

        let bindkeys_only = zshrc_block(
            None,
            Chord {
                ctrl: false,
                alt: false,
                key: ChordKey::Function(5),
            },
            Chord::ctrl('n'),
        );
        assert!(!bindkeys_only.contains("source"));
        assert!(bindkeys_only.contains("bindkey \"${terminfo[kf5]}\" navigate"));
    }

    #[test]
    fn wrapper_candidates_cover_brew_and_cargo_layouts() {
        let brew = wrapper_script_candidates(Path::new(
            "/opt/homebrew/Cellar/navgator/0.4.0/bin/navgator",
        ));
        assert_eq!(
            brew.first(),
            Some(&PathBuf::from(
                "/opt/homebrew/opt/navgator/share/navgator/navgator.zsh"
            )),
            "opt path must come before the versioned Cellar path"
        );
        assert!(brew.contains(&PathBuf::from(
            "/opt/homebrew/Cellar/navgator/0.4.0/share/navgator/navgator.zsh"
        )));
        let cargo = wrapper_script_candidates(Path::new("/repo/target/release/navgator"));
        assert!(cargo.contains(&PathBuf::from("/repo/scripts/navgator.zsh")));
    }
}
