// A form that the config JSON schema drives. The schema gives the structure,
// titles, descriptions, enums and required keys. `stringWidget` and
// `mapKeyWidget` add completions for values that the schema types only as
// strings (paths, action ids, key chords).

import { useEffect, useId, useState, type ReactNode } from "react";
import { api } from "./api";
import type { SettingsView } from "./api/SettingsView";
import { chordOf } from "./keys";

export type Json = null | boolean | number | string | Json[] | JsonObject;
export type JsonObject = { [key: string]: Json };

export type Schema = {
  $ref?: string;
  type?: string | string[];
  title?: string;
  description?: string;
  enum?: Json[];
  anyOf?: Schema[];
  oneOf?: Schema[];
  properties?: Record<string, Schema>;
  additionalProperties?: Schema | boolean;
  items?: Schema;
  required?: string[];
  minimum?: number;
  definitions?: Record<string, Schema>;
};

export type FormContext = { root: Schema; view: SettingsView; filter: string };

type Path = (string | number)[];
/// `undefined` removes the key from the config file.
type Change<T> = (value: T | undefined) => void;

// ------------------------------------------------------------------ schema

/// Follow `$ref` and drop the `null` option of optional values. The outer
/// title and description win over the referenced ones.
export function resolve(schema: Schema, root: Schema): Schema {
  let current = schema;
  const options = current.anyOf?.filter((option) => option.type !== "null");
  if (options?.length === 1 && options[0]) {
    const { anyOf: _, ...outer } = current;
    current = { ...options[0], ...outer };
  }
  if (current.$ref) {
    const name = current.$ref.split("/").pop() ?? "";
    const target = root.definitions?.[name];
    if (!target) throw new Error(`The config schema has no definition ${current.$ref}`);
    const { $ref: _, ...outer } = current;
    current = { ...resolve(target, root), ...outer };
  }
  if (Array.isArray(current.type)) {
    const types = current.type.filter((type) => type !== "null");
    const [only] = types;
    current = { ...current, type: types.length === 1 && only !== undefined ? only : types };
  }
  return current;
}

function isObject(value: Json | undefined): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/// The `type` names of a tagged union such as an action item.
function variantName(variant: Schema): string | null {
  const tag = variant.properties?.type?.enum?.[0];
  return typeof tag === "string" ? tag : null;
}

function isDeprecated(schema: Schema): boolean {
  return (schema.description ?? "").includes("Deprecated");
}

/// A starting value with every required key.
function initialValue(schema: Schema, root: Schema): Json {
  const resolved = resolve(schema, root);
  if (resolved.enum?.[0] !== undefined) return resolved.enum[0];
  switch (resolved.type) {
    case "boolean":
      return false;
    case "integer":
    case "number":
      return resolved.minimum ?? 0;
    case "array":
      return [];
    case "object": {
      const value: JsonObject = {};
      const variant = resolved.oneOf?.[0];
      for (const source of [resolved, variant]) {
        for (const key of source?.required ?? []) {
          const property = source?.properties?.[key];
          if (property) value[key] = initialValue(property, root);
        }
      }
      return value;
    }
    default:
      return "";
  }
}

/// True when the key, title or description of this field or of a field
/// below it holds `filter`.
function matches(name: string, schema: Schema, root: Schema, filter: string, depth = 0): boolean {
  if (filter === "") return true;
  const resolved = resolve(schema, root);
  const text = `${name} ${resolved.title ?? ""} ${resolved.description ?? ""}`.toLowerCase();
  if (text.includes(filter)) return true;
  if (depth > 4) return false;
  const children = { ...resolved.properties };
  for (const variant of resolved.oneOf ?? []) Object.assign(children, variant.properties);
  return Object.entries(children).some(([key, child]) => matches(key, child, root, filter, depth + 1));
}

// ------------------------------------------------------------------ widgets

type Widget = "text" | "multiline" | "path" | "action-id" | "binding-target";

function stringWidget(path: Path): Widget {
  const [section, group] = path;
  const last = path[path.length - 1];
  if (section === "paths" && path.length === 3) return "path";
  if (section === "sort" && group === "ignore" && path.length === 4) return "path";
  if (last === "current_dir") return "path";
  if (section === "actions" && group === "picker" && path.length === 3) return "action-id";
  if (section === "keybindings" && path.length === 3) return "binding-target";
  if (last === "shell") return "multiline";
  return "text";
}

function mapKeyWidget(path: Path): "chord" | "text" {
  return path[0] === "keybindings" ? "chord" : "text";
}

function bindingTargets(ctx: FormContext, context: string): string[] {
  const targets = ctx.view.contexts.find((candidate) => candidate.context === context);
  if (!targets) return ["none"];
  return ["none", ...targets.coreActions, ...(targets.configuredActions ? ctx.view.actionIds : [])];
}

function Suggestions({ id, values }: { id: string; values: string[] }) {
  return (
    <datalist id={id}>
      {values.map((value) => (
        <option key={value} value={value} />
      ))}
    </datalist>
  );
}

function PathInput(props: { value: string; onChange: (value: string) => void }) {
  const id = useId();
  const [focused, setFocused] = useState(false);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  useEffect(() => {
    if (!focused) return;
    let current = true;
    const timer = window.setTimeout(() => {
      api.pathCompletions(props.value).then(
        (found) => current && setSuggestions(found.map((suggestion) => suggestion.value)),
        () => current && setSuggestions([]),
      );
    }, 120);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [focused, props.value]);
  return (
    <>
      <input
        className="setting-input"
        list={id}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        value={props.value}
        placeholder="~/path"
        spellCheck={false}
        onChange={(event) => props.onChange(event.target.value)}
      />
      <Suggestions id={id} values={suggestions} />
    </>
  );
}

function StringInput(props: { path: Path; value: string; ctx: FormContext; onChange: (value: string) => void }) {
  const id = useId();
  const widget = stringWidget(props.path);
  if (widget === "path") return <PathInput value={props.value} onChange={props.onChange} />;
  if (widget === "multiline") {
    return (
      <textarea
        className="setting-input"
        rows={Math.min(Math.max(props.value.split("\n").length, 3), 16)}
        value={props.value}
        spellCheck={false}
        onChange={(event) => props.onChange(event.target.value)}
      />
    );
  }
  const options =
    widget === "action-id"
      ? props.ctx.view.actionIds
      : widget === "binding-target"
        ? bindingTargets(props.ctx, String(props.path[1]))
        : [];
  const unknown = options.length > 0 && props.value !== "" && !options.includes(props.value);
  return (
    <>
      <input
        className="setting-input"
        list={options.length > 0 ? id : undefined}
        value={props.value}
        spellCheck={false}
        data-warning={unknown}
        title={unknown ? "Not a known action for this place" : undefined}
        onChange={(event) => props.onChange(event.target.value)}
      />
      {options.length > 0 && <Suggestions id={id} values={options} />}
    </>
  );
}

/// A key chord in gator's form. "Record" turns the next key press into it.
function ChordInput(props: { value: string; onChange: (value: string) => void }) {
  const [recording, setRecording] = useState(false);
  useEffect(() => {
    if (!recording) return;
    const listener = (event: KeyboardEvent) => {
      const chord = chordOf(event);
      if (!chord) return;
      event.preventDefault();
      event.stopPropagation();
      props.onChange(chord);
      setRecording(false);
    };
    window.addEventListener("keydown", listener, true);
    return () => window.removeEventListener("keydown", listener, true);
  }, [recording, props]);
  return (
    <span className="chord-input">
      <input
        className="setting-input"
        value={props.value}
        placeholder="ctrl+k"
        spellCheck={false}
        onChange={(event) => props.onChange(event.target.value)}
      />
      <button type="button" className="small-button" data-active={recording} onClick={() => setRecording(!recording)}>
        {recording ? "Press a key…" : "Record"}
      </button>
    </span>
  );
}

function Segmented<T extends string>(props: {
  options: { value: T; label: string; title?: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <span className="segmented">
      {props.options.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={option.value === props.value}
          title={option.title}
          onClick={() => props.onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </span>
  );
}

// ------------------------------------------------------------------ fields

type ControlProps = {
  name: string;
  schema: Schema;
  path: Path;
  value: Json | undefined;
  required: boolean;
  ctx: FormContext;
  onChange: Change<Json>;
};

/// The input for one value, chosen from its schema type.
function Control(props: ControlProps): ReactNode {
  const schema = resolve(props.schema, props.ctx.root);
  const { value, onChange, required } = props;

  if (schema.enum) {
    const options = schema.enum.filter((option): option is string => typeof option === "string");
    return (
      <select
        className="setting-input"
        value={typeof value === "string" ? value : ""}
        onChange={(event) => onChange(event.target.value === "" ? undefined : event.target.value)}
      >
        {!required && <option value="">Default</option>}
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    );
  }

  switch (schema.type) {
    case "boolean":
      return required ? (
        <input type="checkbox" checked={value === true} onChange={(event) => onChange(event.target.checked)} />
      ) : (
        <Segmented
          options={[
            { value: "default", label: "Default", title: "Not set in the file" },
            { value: "on", label: "On" },
            { value: "off", label: "Off" },
          ]}
          value={value === true ? "on" : value === false ? "off" : "default"}
          onChange={(choice) => onChange(choice === "default" ? undefined : choice === "on")}
        />
      );
    case "integer":
    case "number":
      return (
        <input
          className="setting-input narrow"
          type="number"
          min={schema.minimum}
          step={schema.type === "integer" ? 1 : "any"}
          value={typeof value === "number" ? value : ""}
          placeholder="Default"
          onChange={(event) => onChange(event.target.value === "" ? undefined : Number(event.target.value))}
        />
      );
    case "string":
      return (
        <StringInput
          path={props.path}
          value={typeof value === "string" ? value : ""}
          ctx={props.ctx}
          onChange={(text) => onChange(text === "" && !required ? undefined : text)}
        />
      );
    case "array":
      return <ListControl {...props} schema={schema} value={Array.isArray(value) ? value : undefined} />;
    case "object":
      if (schema.properties || schema.oneOf) {
        return (
          <ObjectFields
            schema={schema}
            path={props.path}
            value={isObject(value) ? value : {}}
            ctx={props.ctx}
            onChange={(object) => onChange(Object.keys(object).length === 0 && !required ? undefined : object)}
          />
        );
      }
      return <MapControl {...props} schema={schema} value={isObject(value) ? value : undefined} />;
    default:
      return <code className="muted">Unsupported setting type</code>;
  }
}

/// One labeled setting with its description and a reset button.
function Field(props: ControlProps) {
  const schema = resolve(props.schema, props.ctx.root);
  const nested = schema.type === "object" || (schema.type === "array" && resolve(schema.items ?? {}, props.ctx.root).type === "object");
  const set = props.value !== undefined;
  const header = (
    <>
      <span className="field-title">{schema.title ?? props.name}</span>
      <code className="field-key">{props.name}</code>
      {props.required && <span className="badge">required</span>}
      {isDeprecated(schema) && <span className="badge warning">deprecated</span>}
      {set && !props.required && (
        <button
          type="button"
          className="link-button"
          title="Remove this key from the file, so the default applies"
          onClick={(event) => {
            event.preventDefault();
            props.onChange(undefined);
          }}
        >
          Reset
        </button>
      )}
    </>
  );
  const help = schema.description && <p className="field-help">{schema.description}</p>;
  if (nested) {
    return (
      <details className="field nested" data-set={set} open={set || props.ctx.filter !== ""}>
        <summary>{header}</summary>
        {help}
        <Control {...props} />
      </details>
    );
  }
  return (
    <div className="field" data-set={set}>
      <div className="field-header">{header}</div>
      {help}
      <Control {...props} />
    </div>
  );
}

export function ObjectFields(props: {
  schema: Schema;
  path: Path;
  value: JsonObject;
  ctx: FormContext;
  onChange: (value: JsonObject) => void;
}) {
  const { root } = props.ctx;
  const schema = resolve(props.schema, root);
  const variants = (schema.oneOf ?? []).flatMap((variant) => {
    const name = variantName(variant);
    return name ? [{ name, variant }] : [];
  });
  const active = variants.find((candidate) => candidate.name === props.value.type);

  const set = (key: string, next: Json | undefined) => {
    const copy = { ...props.value };
    if (next === undefined) delete copy[key];
    else copy[key] = next;
    props.onChange(copy);
  };
  const switchVariant = (name: string) => {
    const next = variants.find((candidate) => candidate.name === name);
    if (!next) return;
    // Keep the shared keys, drop the keys of the old variant.
    const kept: JsonObject = {};
    for (const [key, value] of Object.entries(props.value)) {
      if (schema.properties?.[key]) kept[key] = value;
    }
    const fresh = initialValue({ ...next.variant, type: "object" }, root);
    props.onChange({ ...kept, ...(isObject(fresh) ? fresh : {}), type: name });
  };

  const fields = (properties: Record<string, Schema>, required: string[]) =>
    Object.entries(properties)
      .filter(([key]) => !(variants.length > 0 && key === "type"))
      .filter(([key, child]) => matches(key, child, root, props.ctx.filter))
      .map(([key, child]) => (
        <Field
          key={key}
          name={key}
          schema={child}
          path={[...props.path, key]}
          value={props.value[key]}
          required={required.includes(key)}
          ctx={props.ctx}
          onChange={(next) => set(key, next)}
        />
      ));

  return (
    <div className="fields-group">
      {variants.length > 0 && (
        <div className="field" data-set={active !== undefined}>
          <div className="field-header">
            <span className="field-title">Type</span>
            <code className="field-key">type</code>
            <span className="badge">required</span>
          </div>
          <Segmented
            options={variants.map(({ name, variant }) => ({ value: name, label: name, title: variant.description ?? name }))}
            value={active?.name ?? ""}
            onChange={switchVariant}
          />
          {active?.variant.description && <p className="field-help">{active.variant.description}</p>}
        </div>
      )}
      {fields(schema.properties ?? {}, schema.required ?? [])}
      {active && fields(active.variant.properties ?? {}, active.variant.required ?? [])}
    </div>
  );
}

function move<T>(items: T[], from: number, to: number): T[] {
  if (to < 0 || to >= items.length) return items;
  const copy = [...items];
  const [item] = copy.splice(from, 1);
  if (item !== undefined) copy.splice(to, 0, item);
  return copy;
}

function ItemButtons(props: {
  index: number;
  count: number;
  onMove: (to: number) => void;
  onCopy: (() => void) | null;
  onRemove: () => void;
}) {
  return (
    // The buttons sit in a <summary>; a click must not also open or close it.
    <span className="item-buttons" onClick={(event) => event.preventDefault()}>
      <button type="button" className="small-button" title="Move up" disabled={props.index === 0} onClick={() => props.onMove(props.index - 1)}>
        ↑
      </button>
      <button
        type="button"
        className="small-button"
        title="Move down"
        disabled={props.index === props.count - 1}
        onClick={() => props.onMove(props.index + 1)}
      >
        ↓
      </button>
      {props.onCopy && (
        <button type="button" className="small-button" title="Duplicate" onClick={props.onCopy}>
          ⧉
        </button>
      )}
      <button type="button" className="small-button danger" title="Remove" onClick={props.onRemove}>
        ×
      </button>
    </span>
  );
}

function itemSummary(item: Json, index: number): string {
  if (!isObject(item)) return `Item ${index + 1}`;
  const name = [item.label, item.name, item.id].find((value) => typeof value === "string" && value !== "");
  const icon = typeof item.icon === "string" ? `${item.icon} ` : "";
  const kind = typeof item.type === "string" ? ` · ${item.type}` : "";
  return `${icon}${typeof name === "string" ? name : `Item ${index + 1}`}${kind}`;
}

function ListControl(props: ControlProps & { value: Json[] | undefined }) {
  const { root } = props.ctx;
  const items = props.value ?? [];
  const itemSchema = resolve(props.schema.items ?? {}, root);
  const objects = itemSchema.type === "object";
  const update = (next: Json[]) => props.onChange(next.length === 0 && !props.required ? undefined : next);
  const setItem = (index: number, value: Json | undefined) =>
    update(value === undefined ? items.filter((_, other) => other !== index) : items.map((item, other) => (other === index ? value : item)));
  const buttons = (index: number) => (
    <ItemButtons
      index={index}
      count={items.length}
      onMove={(to) => update(move(items, index, to))}
      onCopy={objects ? () => update([...items.slice(0, index + 1), structuredClone(items[index] ?? null), ...items.slice(index + 1)]) : null}
      onRemove={() => setItem(index, undefined)}
    />
  );

  return (
    <div className="list-control">
      {items.map((item, index) =>
        objects ? (
          <details key={index} className="list-card">
            <summary>
              <span className="list-card-title">{itemSummary(item, index)}</span>
              {buttons(index)}
            </summary>
            <ObjectFields
              schema={itemSchema}
              path={[...props.path, index]}
              value={isObject(item) ? item : {}}
              ctx={{ ...props.ctx, filter: "" }}
              onChange={(next) => setItem(index, next)}
            />
          </details>
        ) : (
          <div key={index} className="list-row">
            <Control
              name={props.name}
              schema={itemSchema}
              path={[...props.path, index]}
              value={item}
              required
              ctx={props.ctx}
              onChange={(next) => setItem(index, next ?? "")}
            />
            {buttons(index)}
          </div>
        ),
      )}
      <button type="button" className="small-button add" onClick={() => update([...items, initialValue(itemSchema, root)])}>
        + Add {objects ? (itemSchema.title ?? "item").toLowerCase() : "entry"}
      </button>
    </div>
  );
}

/// A table with free keys, such as the chords of one keybinding context.
function MapControl(props: ControlProps & { value: JsonObject | undefined }) {
  const entries = Object.entries(props.value ?? {});
  const valueSchema: Schema =
    typeof props.schema.additionalProperties === "object" ? props.schema.additionalProperties : { type: "string" };
  const keyWidget = mapKeyWidget(props.path);
  const [newKey, setNewKey] = useState("");
  const update = (next: [string, Json][]) =>
    props.onChange(next.length === 0 && !props.required ? undefined : Object.fromEntries(next));
  const duplicate = newKey !== "" && entries.some(([key]) => key === newKey);
  const defaults =
    props.path[0] === "keybindings"
      ? (props.ctx.view.contexts.find((candidate) => candidate.context === props.name)?.defaults ?? [])
      : [];

  return (
    <div className="list-control">
      {defaults.length > 0 && (
        <p className="default-bindings">
          <span className="muted">Built-in:</span>
          {defaults.map((binding) => (
            <button
              key={binding.chord}
              type="button"
              className="binding-chip"
              data-overridden={entries.some(([key]) => key === binding.chord)}
              title={`Click to override ${binding.chord}`}
              onClick={() => setNewKey(binding.chord)}
            >
              <kbd>{binding.chord}</kbd> {binding.target}
            </button>
          ))}
        </p>
      )}
      {entries.map(([key, value], index) => (
        <div key={index} className="list-row map-row">
          {keyWidget === "chord" ? (
            <ChordInput value={key} onChange={(next) => update(entries.map((entry, other) => (other === index ? [next, entry[1]] : entry)))} />
          ) : (
            <input
              className="setting-input"
              value={key}
              spellCheck={false}
              onChange={(event) => update(entries.map((entry, other) => (other === index ? [event.target.value, entry[1]] : entry)))}
            />
          )}
          <span className="muted">→</span>
          <Control
            name={key}
            schema={valueSchema}
            path={[...props.path, key]}
            value={value}
            required
            ctx={props.ctx}
            onChange={(next) => update(entries.map((entry, other) => (other === index ? [key, next ?? ""] : entry)))}
          />
          <button type="button" className="small-button danger" title="Remove" onClick={() => update(entries.filter((_, other) => other !== index))}>
            ×
          </button>
        </div>
      ))}
      <div className="list-row map-row">
        {keyWidget === "chord" ? <ChordInput value={newKey} onChange={setNewKey} /> : (
          <input className="setting-input" value={newKey} placeholder="key" spellCheck={false} onChange={(event) => setNewKey(event.target.value)} />
        )}
        <button
          type="button"
          className="small-button add"
          disabled={newKey === "" || duplicate}
          title={duplicate ? "This key is already in the list" : undefined}
          onClick={() => {
            update([...entries, [newKey, initialValue(valueSchema, props.ctx.root)]]);
            setNewKey("");
          }}
        >
          + Add
        </button>
      </div>
    </div>
  );
}

/// One top-level section of the config file.
export function Section(props: { name: string; ctx: FormContext; value: Json | undefined; onChange: Change<Json> }) {
  const schema = props.ctx.root.properties?.[props.name];
  if (!schema) return null;
  const resolved = resolve(schema, props.ctx.root);
  return (
    <section className="settings-section" id={`section-${props.name}`}>
      <h2>
        {resolved.title ?? props.name} <code className="field-key">[{props.name}]</code>
      </h2>
      {resolved.description && <p className="field-help">{resolved.description}</p>}
      <Control
        name={props.name}
        schema={schema}
        path={[props.name]}
        value={props.value}
        required={false}
        ctx={props.ctx}
        onChange={props.onChange}
      />
    </section>
  );
}

export function sectionMatches(name: string, ctx: FormContext): boolean {
  const schema = ctx.root.properties?.[name];
  return schema !== undefined && matches(name, schema, ctx.root, ctx.filter);
}
