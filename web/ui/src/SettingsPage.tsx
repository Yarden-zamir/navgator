import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { SettingsView } from "./api/SettingsView";
import { Section, resolve, sectionMatches, type FormContext, type Json, type JsonObject, type Schema } from "./SchemaForm";

/// The order of the sections in the navigation. Sections that the schema
/// adds later and that are not in this list come last.
const SECTION_ORDER = ["paths", "actions", "create", "keybindings", "sort", "preview", "remote", "branches", "ui"];

export function openSettings() {
  window.location.hash = "settings";
}

type Tab = "form" | "toml";
type Status = { text: string; error: boolean };

function sectionNames(schema: Schema): string[] {
  const names = Object.keys(schema.properties ?? {}).filter((name) => name !== "$schema");
  const rank = (name: string) => {
    const index = SECTION_ORDER.indexOf(name);
    return index === -1 ? SECTION_ORDER.length : index;
  };
  return names.sort((left, right) => rank(left) - rank(right));
}

function same(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

export function SettingsPage({ onClose }: { onClose: () => void }) {
  const [view, setView] = useState<SettingsView | null>(null);
  const [draft, setDraft] = useState<JsonObject>({});
  const [text, setText] = useState("");
  const [tab, setTab] = useState<Tab>("form");
  const [filter, setFilter] = useState("");
  const [status, setStatus] = useState<Status | null>(null);
  const [saving, setSaving] = useState(false);

  const show = useCallback((loaded: SettingsView) => {
    setView(loaded);
    // The server sends the file parsed from TOML, so every value is JSON.
    setDraft(loaded.values as JsonObject);
    setText(loaded.text);
  }, []);

  useEffect(() => {
    api.settings().then(show, (error: unknown) => setStatus({ text: String(error), error: true }));
  }, [show]);

  const dirty = view !== null && (tab === "form" ? !same(draft, view.values) : text !== view.text);

  const save = useCallback(async () => {
    if (!dirty || saving) return;
    setSaving(true);
    try {
      const saved = await api.saveSettings(tab === "form" ? { kind: "values", values: draft } : { kind: "text", text });
      show(saved);
      setStatus({ text: "Saved. navgator loaded the new config.", error: false });
    } catch (error) {
      setStatus({ text: error instanceof Error ? error.message : String(error), error: true });
    } finally {
      setSaving(false);
    }
  }, [dirty, saving, tab, draft, text, show]);

  const discard = () => {
    if (view) show(view);
    setStatus(null);
  };

  const close = useCallback(() => {
    if (!dirty || window.confirm("Close settings and drop the changes that are not saved?")) onClose();
  }, [dirty, onClose]);

  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "s") {
        event.preventDefault();
        void save();
      } else if (event.key === "Escape" && !event.defaultPrevented) {
        close();
      }
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [save, close]);

  if (!view) {
    return (
      <div className="settings-page">
        <p className={status?.error ? "error-text" : "loading"}>{status?.text ?? "Loading settings..."}</p>
      </div>
    );
  }

  const root = view.schema as Schema;
  const ctx: FormContext = { root, view, filter: filter.trim().toLowerCase() };
  const sections = sectionNames(root).filter((name) => sectionMatches(name, ctx));
  const switchTab = (next: Tab) => {
    if (next === tab) return;
    if (dirty) {
      setStatus({ text: "Save or discard the changes before you switch views.", error: true });
      return;
    }
    setTab(next);
  };

  return (
    <div className="settings-page">
      <header className="settings-header panel">
        <div className="settings-title">
          <h1>Settings</h1>
          <code className="muted" title="The file that this page edits">
            {view.file}
          </code>
        </div>
        <span className="segmented">
          <button type="button" aria-pressed={tab === "form"} onClick={() => switchTab("form")}>
            Form
          </button>
          <button type="button" aria-pressed={tab === "toml"} onClick={() => switchTab("toml")}>
            TOML
          </button>
        </span>
        <span className="settings-actions">
          {dirty && <span className="badge warning">not saved</span>}
          <button type="button" className="small-button" disabled={!dirty || saving} onClick={discard}>
            Discard
          </button>
          <button type="button" className="small-button primary" disabled={!dirty || saving} onClick={() => void save()}>
            {saving ? "Saving..." : "Save"} <kbd>⌘S</kbd>
          </button>
          <button type="button" className="small-button" onClick={close}>
            Close <kbd>Esc</kbd>
          </button>
        </span>
      </header>

      {status && (
        <p className="settings-status" data-error={status.error}>
          {status.text}
        </p>
      )}
      {view.otherFiles.length > 0 && (
        <p className="field-help">
          navgator also loads {view.otherFiles.join(", ")}. A file later in the load order wins for the same setting.
        </p>
      )}

      {tab === "toml" ? (
        <textarea
          className="settings-toml panel mono"
          value={text}
          spellCheck={false}
          onChange={(event) => setText(event.target.value)}
        />
      ) : (
        <div className="settings-body">
          <nav className="settings-nav panel">
            <input
              className="setting-input"
              value={filter}
              placeholder="Search settings"
              spellCheck={false}
              autoFocus
              onChange={(event) => setFilter(event.target.value)}
            />
            {sections.map((name) => {
              const schema = root.properties?.[name];
              const title = schema ? (resolve(schema, root).title ?? name) : name;
              const changed = !same(draft[name], view.values[name]);
              return (
                <a key={name} href={`#settings/${name}`} onClick={(event) => {
                  event.preventDefault();
                  document.getElementById(`section-${name}`)?.scrollIntoView({ behavior: "smooth" });
                }}>
                  {title}
                  {changed && <span className="dot" title="Changed, not saved" />}
                </a>
              );
            })}
          </nav>
          <div className="settings-form panel">
            {sections.length === 0 && <p className="muted">No setting matches "{filter}".</p>}
            {sections.map((name) => (
              <Section
                key={name}
                name={name}
                ctx={ctx}
                value={draft[name]}
                onChange={(value: Json | undefined) => {
                  const copy = { ...draft };
                  if (value === undefined) delete copy[name];
                  else copy[name] = value;
                  setDraft(copy);
                }}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
