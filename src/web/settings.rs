//! The settings panel: read and edit the user config file. A form edit
//! rewrites only the values that change, so the comments and the layout of
//! every other value stay.

use super::api::{blocking, ApiError, ApiResult};
use super::app::App;
use crate::config::{config_file_paths, config_schema_json, default_user_config_path};
use crate::model::keybindings::{
    default_keymap, target_is_compatible, BindingContext, BindingTarget, CoreAction,
};
use axum::{extract::State, Json};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike};

type AppState = State<Arc<App>>;

/// Only one edit at a time reads, writes and reloads the config file.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// The JSON schema of the config file (`navgator config-schema`).
    #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
    pub schema: Value,
    /// The file that the panel edits.
    pub file: String,
    pub text: String,
    /// The file as JSON. Keys that the file does not set are absent.
    #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
    pub values: Value,
    /// Other config files that navgator also loads.
    pub other_files: Vec<String>,
    pub contexts: Vec<ContextTargets>,
    /// Ids of the configured actions, for keybindings and the picker list.
    pub action_ids: Vec<String>,
}

/// The core actions that a keybinding context accepts.
#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ContextTargets {
    pub context: BindingContext,
    pub core_actions: Vec<CoreAction>,
    pub configured_actions: bool,
    /// The built-in bindings, which a config chord for the same key replaces.
    pub defaults: Vec<DefaultBinding>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct DefaultBinding {
    pub chord: String,
    pub target: String,
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SettingsUpdate {
    /// The whole file as JSON, as `SettingsView.values` has it.
    Values {
        #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
        values: Value,
    },
    /// The whole file as TOML text.
    Text { text: String },
}

pub async fn read(State(app): AppState) -> ApiResult<SettingsView> {
    blocking(move || view(&app))
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

/// Write the file, then reload the config. When the new config does not
/// load, put the old file back and answer with the load error.
pub async fn update(
    State(app): AppState,
    Json(update): Json<SettingsUpdate>,
) -> ApiResult<SettingsView> {
    blocking(move || {
        let _guard = WRITE_LOCK
            .lock()
            .expect("navgator settings write lock is poisoned");
        let path = edited_file()?;
        let previous = read_file(&path)?;
        let next = match update {
            SettingsUpdate::Values { values } => apply_values(&previous, &values)?,
            SettingsUpdate::Text { text } => text,
        };
        write_file(&path, &next)?;
        if let Err(error) = app.reload() {
            write_file(&path, &previous)?;
            return Err(error);
        }
        view(&app)
    })
    .await
    .map(Json)
    .map_err(ApiError::bad_request)
}

fn view(app: &App) -> Result<SettingsView, String> {
    let home = home_dir()?;
    let path = default_user_config_path(&home);
    let text = read_file(&path)?;
    let schema = config_schema_json().map_err(|error| error.to_string())?;
    let schema = serde_json::from_str(&schema)
        .map_err(|error| format!("the config schema is not JSON: {error}"))?;
    let defaults = default_keymap();
    let contexts = BindingContext::ORDERED
        .iter()
        .map(|&context| ContextTargets {
            defaults: defaults
                .bindings_for_context(context)
                .iter()
                .map(|binding| DefaultBinding {
                    chord: binding.chord.to_string(),
                    target: binding.target.as_str().to_string(),
                })
                .collect(),
            context,
            core_actions: CoreAction::ALL
                .into_iter()
                .filter(|&action| target_is_compatible(context, &BindingTarget::Core(action)))
                .collect(),
            configured_actions: target_is_compatible(
                context,
                &BindingTarget::Configured(String::new()),
            ),
        })
        .collect();
    let action_ids = app
        .settings()
        .actions
        .items
        .iter()
        .filter_map(|action| action.id.clone())
        .collect();
    Ok(SettingsView {
        schema,
        file: path.display().to_string(),
        values: toml_to_json(&text)?,
        text,
        other_files: config_file_paths(&home)
            .into_iter()
            .filter(|other| other != &path && other.is_file())
            .map(|other| other.display().to_string())
            .collect(),
        contexts,
        action_ids,
    })
}

/// The file that the panel edits: `$NAVGATOR_CONFIG`, else the user config.
fn edited_file() -> Result<PathBuf, String> {
    Ok(default_user_config_path(&home_dir()?))
}

fn home_dir() -> Result<PathBuf, String> {
    gator::config::home_dir().map_err(|error| format!("cannot find the home folder: {error}"))
}

/// The file text, or an empty text when the file does not exist.
fn read_file(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn toml_to_json(text: &str) -> Result<Value, String> {
    Figment::from(Toml::string(text))
        .extract()
        .map_err(|error| format!("the config file is not valid TOML: {error}"))
}

/// Rewrite `text` so that it holds `values`. Values that do not change keep
/// their text, comments and position.
fn apply_values(text: &str, values: &Value) -> Result<String, String> {
    let Value::Object(new) = values else {
        return Err("settings values must be a table".to_string());
    };
    let mut document: DocumentMut = text
        .parse()
        .map_err(|error| format!("the config file is not valid TOML: {error}"))?;
    let old = toml_to_json(text)?;
    let empty = Map::new();
    merge_table(
        document.as_table_mut(),
        old.as_object().unwrap_or(&empty),
        new,
        false,
    )?;
    Ok(document.to_string())
}

/// `inline` is true inside an inline table or array, where TOML allows only
/// inline values.
fn merge_table(
    table: &mut dyn TableLike,
    old: &Map<String, Value>,
    new: &Map<String, Value>,
    inline: bool,
) -> Result<(), String> {
    let removed = table
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| new.get(key).is_none_or(Value::is_null))
        .collect::<Vec<_>>();
    for key in removed {
        table.remove(&key);
    }
    for (key, value) in new {
        if value.is_null() {
            continue;
        }
        match (table.get_mut(key), old.get(key)) {
            (Some(item), Some(old_value)) => merge_item(item, old_value, value, inline)?,
            _ => {
                table.insert(key, new_item(value, inline)?);
            }
        }
    }
    Ok(())
}

fn merge_item(item: &mut Item, old: &Value, new: &Value, inline: bool) -> Result<(), String> {
    if old == new {
        return Ok(());
    }
    let inline = inline || item.is_value();
    match (old, new) {
        (Value::Object(old_map), Value::Object(new_map)) => {
            if let Some(table) = item.as_table_like_mut() {
                return merge_table(table, old_map, new_map, inline);
            }
        }
        (Value::Array(old_items), Value::Array(new_items)) => {
            if let Some(tables) = item.as_array_of_tables_mut() {
                if tables.len() == old_items.len() {
                    return merge_array_of_tables(tables, old_items, new_items);
                }
            }
        }
        _ => {}
    }
    let mut replacement = new_item(new, inline)?;
    // Keep a comment after the old value, for example `key = 1 # note`.
    if let (Some(old_value), Item::Value(new_value)) = (item.as_value(), &mut replacement) {
        *new_value.decor_mut() = old_value.decor().clone();
    }
    *item = replacement;
    Ok(())
}

/// Reuse an old table for each new element, so its comments move with it:
/// an equal old element first, then one with the same `id`, then the one at
/// the same position.
fn merge_array_of_tables(
    tables: &mut ArrayOfTables,
    old: &[Value],
    new: &[Value],
) -> Result<(), String> {
    let mut unused: Vec<Option<Table>> = tables.iter().cloned().map(Some).collect();
    let mut merged = ArrayOfTables::new();
    for (index, value) in new.iter().enumerate() {
        let Value::Object(new_map) = value else {
            return Err("a list of tables can hold only tables".to_string());
        };
        let free = |candidate: &usize| unused[*candidate].is_some();
        let id = new_map.get("id").filter(|id| !id.is_null());
        let pick = (0..old.len())
            .filter(free)
            .find(|&candidate| old[candidate] == *value)
            .or_else(|| {
                (0..old.len())
                    .filter(free)
                    .find(|&candidate| id.is_some() && old[candidate].get("id") == id)
            })
            .or_else(|| (index < old.len() && unused[index].is_some()).then_some(index));
        let table = match pick.and_then(|candidate| Some((candidate, unused[candidate].take()?))) {
            Some((candidate, mut table)) => {
                let empty = Map::new();
                let old_map = old[candidate].as_object().unwrap_or(&empty);
                merge_table(&mut table, old_map, new_map, false)?;
                table
            }
            None => {
                let mut table = Table::new();
                merge_table(&mut table, &Map::new(), new_map, false)?;
                table
            }
        };
        merged.push(table);
    }
    *tables = merged;
    Ok(())
}

/// A new TOML item for `value`. Outside inline context, tables become
/// `[table]` sections and lists of tables become `[[table]]` sections.
fn new_item(value: &Value, inline: bool) -> Result<Item, String> {
    if !inline {
        match value {
            Value::Object(map) => {
                let mut table = Table::new();
                merge_table(&mut table, &Map::new(), map, false)?;
                // A table that holds only tables needs no `[header]` line.
                let only_tables = table
                    .iter()
                    .all(|(_, item)| item.is_table() || item.is_array_of_tables());
                table.set_implicit(only_tables);
                return Ok(Item::Table(table));
            }
            Value::Array(items) if !items.is_empty() && items.iter().all(Value::is_object) => {
                let mut tables = ArrayOfTables::new();
                for item in items {
                    let Item::Table(table) = new_item(item, false)? else {
                        unreachable!("a JSON object becomes a table");
                    };
                    tables.push(table);
                }
                return Ok(Item::ArrayOfTables(tables));
            }
            _ => {}
        }
    }
    Ok(Item::Value(toml_value(value)?))
}

fn toml_value(value: &Value) -> Result<toml_edit::Value, String> {
    Ok(match value {
        Value::Null => return Err("a list cannot hold an empty value".to_string()),
        Value::Bool(value) => (*value).into(),
        Value::Number(number) => match number.as_i64() {
            Some(integer) => integer.into(),
            None => number
                .as_f64()
                .ok_or_else(|| format!("{number} is not a TOML number"))?
                .into(),
        },
        Value::String(text) => text.as_str().into(),
        Value::Array(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(toml_value(item)?);
            }
            array.into()
        }
        Value::Object(map) => {
            let mut table = InlineTable::new();
            for (key, item) in map {
                if !item.is_null() {
                    table.insert(key, toml_value(item)?);
                }
            }
            table.into()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{apply_values, toml_to_json};
    use serde_json::json;

    const CONFIG: &str = r#"# My navgator config
[paths]
index_folders = ["/a"] # main folder

# Sort settings
[sort]
default = "recents"

# First action
[[actions.items]]
id = "one"
label = "One"
type = "navigate"

# Second action
[[actions.items]]
id = "two"
label = "Two"
type = "navigate"
"#;

    fn edit(change: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut values = toml_to_json(CONFIG).expect("valid config");
        change(&mut values);
        let text = apply_values(CONFIG, &values).expect("edit applies");
        assert_eq!(toml_to_json(&text).expect("valid result"), values);
        text
    }

    #[test]
    fn unchanged_values_keep_the_text() {
        assert_eq!(edit(|_| {}), CONFIG);
    }

    #[test]
    fn a_changed_value_keeps_the_comments() {
        let text = edit(|values| values["paths"]["index_folders"] = json!(["/a", "/b"]));
        assert!(text.contains("# My navgator config"));
        assert!(text.contains("# main folder"));
        assert!(text.contains("# Sort settings"));
    }

    #[test]
    fn null_removes_a_key_and_new_tables_get_sections() {
        let text = edit(|values| {
            values["sort"]
                .as_object_mut()
                .expect("sort table")
                .remove("default");
            values["ui"] = json!({ "theme": "dark" });
        });
        assert!(!text.contains("recents"));
        assert!(text.contains("[ui]"));
    }

    #[test]
    fn a_table_of_tables_gets_no_empty_header() {
        let text = edit(|values| {
            values["keybindings"] = json!({ "navigator": { "ctrl+k": "copy-path" } });
        });
        assert!(text.contains("[keybindings.navigator]"));
        assert!(!text.contains("[keybindings]\n"));
    }

    #[test]
    fn a_removed_list_table_takes_its_comment_along() {
        let text = edit(|values| {
            let items = values["actions"]["items"].as_array_mut().expect("items");
            items.remove(0);
        });
        assert!(!text.contains("# First action"));
        assert!(text.contains("# Second action"));
    }
}
