import { useEffect, useRef, type ReactNode } from "react";
import type { ActionView } from "./api/ActionView";
import type { CreateView } from "./api/CreateView";
import type { KeyContext } from "./api/KeyContext";
import type { KeyTarget } from "./api/KeyTarget";
import type { PathSuggestion } from "./api/PathSuggestion";

/// `onClose` is null while the overlay cannot close, for example during a
/// running task.
function Modal(props: { title: string; children: ReactNode; footer: ReactNode; onClose: (() => void) | null }) {
  const { title, children, footer, onClose } = props;
  return (
    <div
      className="backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose?.();
      }}
    >
      <div className="modal" role="dialog" aria-label={title}>
        <header className="panel-title">
          <span>{title}</span>
          {onClose && (
            <button type="button" className="modal-close" title="Close" onClick={onClose}>
              ×
            </button>
          )}
        </header>
        <div className="modal-body">{children}</div>
        <footer className="modal-footer">{footer}</footer>
      </div>
    </div>
  );
}

type PickerItem = { key: string; icon: string | null; label: string };

function PickerList(props: {
  items: PickerItem[];
  selected: number;
  query: string;
  empty: string;
  onPick: (index: number) => void;
}) {
  const listRef = useRef<HTMLOListElement>(null);
  useEffect(() => {
    listRef.current?.children[props.selected]?.scrollIntoView({ block: "nearest" });
  }, [props.selected]);
  return (
    <>
      <div className="picker-query">{props.query || <span className="muted">Type to filter</span>}</div>
      <ol className="picker" ref={listRef}>
        {props.items.length === 0 && <li className="empty">{props.empty}</li>}
        {props.items.map((item, index) => (
          <li
            key={item.key}
            className="picker-row"
            aria-selected={index === props.selected}
            onClick={() => props.onPick(index)}
          >
            <span className="icon">{item.icon ?? ""}</span>
            {item.label}
          </li>
        ))}
      </ol>
    </>
  );
}

export function ActionPicker(props: {
  path: string;
  actions: ActionView[];
  selected: number;
  query: string;
  footer: ReactNode;
  onPick: (index: number) => void;
  onClose: () => void;
}) {
  return (
    <Modal title={`Actions · ${props.path}`} footer={props.footer} onClose={props.onClose}>
      <PickerList
        items={props.actions.map((action) => ({
          key: String(action.index),
          icon: action.icon,
          label: action.label,
        }))}
        selected={props.selected}
        query={props.query}
        empty="No actions for this path"
        onPick={props.onPick}
      />
    </Modal>
  );
}

export function CreatePicker(props: {
  items: CreateView[];
  selected: number;
  query: string;
  footer: ReactNode;
  onPick: (index: number) => void;
  onClose: () => void;
}) {
  return (
    <Modal title="Create" footer={props.footer} onClose={props.onClose}>
      <PickerList
        items={props.items.map((item) => ({ key: String(item.index), icon: item.icon, label: item.label }))}
        selected={props.selected}
        query={props.query}
        empty="No create recipes match"
        onPick={props.onPick}
      />
    </Modal>
  );
}

export type CreateFormState = {
  index: number;
  path: string | null;
  values: string[];
  active: number;
  error: string | null;
  suggestions: PathSuggestion[];
  suggestion: number;
  completions: boolean;
};

export function CreateForm(props: {
  item: CreateView;
  form: CreateFormState;
  footer: ReactNode;
  onInput: (value: string) => void;
  onActivate: (prompt: number) => void;
  onPickSuggestion: (index: number) => void;
  onClose: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
  }, [props.form.active]);
  return (
    <Modal title={`${props.item.icon ?? ""} ${props.item.label}`.trim()} footer={props.footer} onClose={props.onClose}>
      <div className="create-form">
        <div className="fields">
          {props.item.prompts.map((prompt, index) => (
            <label key={prompt.name} className="field" data-active={index === props.form.active}>
              <span>
                {prompt.label}
                {prompt.required && " *"}
              </span>
              {index === props.form.active ? (
                <input
                  ref={inputRef}
                  value={props.form.values[index] ?? ""}
                  spellCheck={false}
                  autoComplete="off"
                  onChange={(event) => props.onInput(event.target.value)}
                />
              ) : (
                <input
                  value={props.form.values[index] ?? ""}
                  readOnly
                  tabIndex={-1}
                  onFocus={() => props.onActivate(index)}
                />
              )}
            </label>
          ))}
          {props.form.error && <p className="error-text">{props.form.error}</p>}
        </div>
        {props.form.suggestions.length > 0 && (
          <ol className="picker completions" data-focused={props.form.completions}>
            {props.form.suggestions.map((suggestion, index) => (
              <li
                key={suggestion.value}
                className="picker-row"
                aria-selected={props.form.completions && index === props.form.suggestion}
                onMouseDown={(event) => {
                  event.preventDefault();
                  props.onPickSuggestion(index);
                }}
              >
                {suggestion.display}
              </li>
            ))}
          </ol>
        )}
      </div>
    </Modal>
  );
}

export function ProgressOverlay(props: {
  title: string;
  messages: string[];
  error: string | null;
  footer: ReactNode;
  onClose: () => void;
}) {
  return (
    <Modal title={props.title} footer={props.footer} onClose={props.error === null ? null : props.onClose}>
      <div className="progress" data-error={props.error !== null}>
        {props.messages.map((message, index) => (
          <pre key={index}>{message}</pre>
        ))}
        {props.error === null ? (
          <div className="progress-bar" />
        ) : (
          <pre className="error-text">{props.error}</pre>
        )}
      </div>
    </Modal>
  );
}

function targetText(target: KeyTarget, actions: ActionView[]): string {
  if (target.kind === "disabled") return "(off)";
  if (target.kind === "core") return target.action.replaceAll("-", " ");
  return actions.find((action) => action.id === target.id)?.label ?? target.id;
}

/// Every binding of every context, from the keymap that the config builds.
export function KeysHelp(props: { keymap: KeyContext[]; actions: ActionView[]; footer: ReactNode; onClose: () => void }) {
  return (
    <Modal title="Keys" footer={props.footer} onClose={props.onClose}>
      <div className="keys-help">
        {props.keymap
          .filter((context) => context.bindings.length > 0)
          .map((context) => (
            <section key={context.context}>
              <h3>{context.context.replaceAll("-", " ")}</h3>
              <table>
                <tbody>
                  {context.bindings.map((binding) => (
                    <tr key={binding.chord} data-kind={binding.target.kind}>
                      <td>
                        <kbd>{binding.label}</kbd>
                      </td>
                      <td>{targetText(binding.target, props.actions)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          ))}
      </div>
    </Modal>
  );
}
