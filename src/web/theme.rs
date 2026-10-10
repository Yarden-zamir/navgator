//! Follow the active T3 Code theme, so the page looks native in the T3
//! preview and stays consistent in a plain browser.
//!
//! T3 keeps the theme name in `~/.t3/userdata/settings.json` (`defaultTheme`)
//! and user themes in `~/.t3/userdata/themes/<name>.json`. The page uses the
//! same CSS variable names as T3 (`--app-theme-*`, `--radius`, ...).
// Limit: built-in T3 themes have no JSON file, so the page keeps its own
// defaults for them. Revisit if T3 starts to ship built-in themes as files.

use serde_json::{Map, Value};
use std::{fs, path::Path};

/// The `:root` rule for the active T3 theme, or `None` when no user theme is active.
pub fn t3_theme_css(home: &Path) -> Result<Option<String>, String> {
    let userdata = home.join(".t3").join("userdata");
    let settings_path = userdata.join("settings.json");
    let Some(settings) = read_json(&settings_path)? else {
        return Ok(None);
    };
    let Some(name) = settings.get("defaultTheme").and_then(Value::as_str) else {
        return Ok(None);
    };
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(format!(
            "unexpected T3 theme name {name:?} in {}",
            settings_path.display()
        ));
    }
    let Some(theme) = read_json(&userdata.join("themes").join(format!("{name}.json")))? else {
        return Ok(None);
    };
    Ok(Some(theme_css(&theme)))
}

fn read_json(path: &Path) -> Result<Option<Value>, String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    serde_json::from_str(&contents)
        .map(Some)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn theme_css(theme: &Value) -> String {
    let mut declarations = Vec::new();
    let mut push = |name: String, value: &str| {
        // Theme files are local and trusted, but a value must never end the rule.
        if !value.contains([';', '{', '}', '<']) {
            declarations.push(format!("  {name}: {value};"));
        }
    };

    let appearance = match theme.get("appearance").and_then(Value::as_str) {
        Some("dark") => "dark",
        _ => "light",
    };
    push("color-scheme".to_string(), appearance);
    for key in ["canvas", "accent"] {
        if let Some(value) = theme.get(key).and_then(Value::as_str) {
            push(format!("--app-theme-{key}"), value);
        }
    }
    if let Some(colors) = theme.get("colors").and_then(Value::as_object) {
        for (key, value) in colors {
            if let Some(value) = value.as_str() {
                push(format!("--app-theme-{}", kebab_case(key)), value);
            }
        }
    }
    let string = |key: &str| theme.get(key).and_then(Value::as_str);
    if let Some(value) = string("radius") {
        push("--radius".to_string(), value);
    }
    if let Some(value) = string("borderWidth") {
        push("--theme-border-width".to_string(), value);
    }
    if let Some(value) = string("shadow") {
        push("--theme-shadow".to_string(), value);
    }
    if let Some(value) = string("canvasPatternColor") {
        push("--theme-canvas-pattern-color".to_string(), value);
    }
    if let Some(value) = string("canvasPattern") {
        push("--theme-canvas-pattern".to_string(), value);
    }
    let fonts = theme.get("fonts").and_then(Value::as_object);
    for (key, fallback) in [
        ("sans", "system-ui, sans-serif"),
        ("code", "ui-monospace, monospace"),
    ] {
        if let Some(family) = fonts
            .and_then(|fonts: &Map<String, Value>| fonts.get(key))
            .and_then(Value::as_str)
        {
            push(
                format!("--font-{key}"),
                &format!("\"{family}\", {fallback}"),
            );
        }
    }
    format!(":root {{\n{}\n}}\n", declarations.join("\n"))
}

fn kebab_case(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::theme_css;
    use serde_json::json;

    #[test]
    fn maps_theme_fields_to_t3_variable_names() {
        let css = theme_css(&json!({
            "appearance": "dark",
            "canvas": "#15122B",
            "colors": { "surfaceRaised": "#1D1938", "text": "#F3F0FF" },
            "radius": "0.25rem",
            "borderWidth": "2px",
            "shadow": "4px 4px 0 0 #B9A8FF",
            "fonts": { "sans": "Bricolage Grotesque" }
        }));
        assert!(css.contains("color-scheme: dark;"));
        assert!(css.contains("--app-theme-canvas: #15122B;"));
        assert!(css.contains("--app-theme-surface-raised: #1D1938;"));
        assert!(css.contains("--theme-border-width: 2px;"));
        assert!(css.contains("--font-sans: \"Bricolage Grotesque\", system-ui, sans-serif;"));
    }

    #[test]
    fn drops_values_that_could_end_the_rule() {
        let css = theme_css(&json!({ "colors": { "text": "red; } body { display: none" } }));
        assert!(!css.contains("display: none"));
    }
}
