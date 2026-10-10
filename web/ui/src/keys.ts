// Resolve browser key presses against the navgator keymap from the config.
// Chords use gator's canonical form: modifiers in the order ctrl, alt, shift,
// super, then the key name, joined by "+" (for example "ctrl+shift+tab").

import type { BindingContext } from "./api/BindingContext";
import type { Bootstrap } from "./api/Bootstrap";
import type { CoreAction } from "./api/CoreAction";
import type { KeyTarget } from "./api/KeyTarget";

const NAMED_KEYS: Record<string, string> = {
  Enter: "enter",
  Tab: "tab",
  Escape: "esc",
  Backspace: "backspace",
  Delete: "delete",
  Insert: "insert",
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  CapsLock: "caps-lock",
  ScrollLock: "scroll-lock",
  NumLock: "num-lock",
  PrintScreen: "print-screen",
  Pause: "pause",
  ContextMenu: "menu",
  " ": "space",
};

const PUNCTUATION: Record<string, string> = {
  "+": "plus",
  "=": "equals",
  ":": "colon",
  ";": "semicolon",
  ",": "comma",
  ".": "period",
  "-": "minus",
  "/": "slash",
  "\\": "backslash",
  "'": "quote",
  "`": "backtick",
  "[": "left-bracket",
  "]": "right-bracket",
};

/** The key part of a chord, and whether Shift belongs in the chord. */
function keyName(event: KeyboardEvent): { name: string; shiftCounts: boolean } | null {
  const named = NAMED_KEYS[event.key];
  if (named) return { name: named, shiftCounts: true };
  const functionNumber = event.key.startsWith("F") ? Number(event.key.slice(1)) : Number.NaN;
  if (Number.isInteger(functionNumber) && functionNumber >= 1 && functionNumber <= 35) {
    return { name: `f${functionNumber}`, shiftCounts: true };
  }
  // Alt (Option on macOS) changes `key`, so letters and digits come from `code`.
  if (event.code.startsWith("Key") && event.code.length === 4) {
    return { name: event.code.slice(3).toLowerCase(), shiftCounts: true };
  }
  if (event.code.startsWith("Digit") && (event.altKey || event.ctrlKey || event.metaKey)) {
    return { name: event.code.slice(5), shiftCounts: false };
  }
  if ([...event.key].length !== 1) return null;
  // Terminals report symbols such as "?" without Shift, so Shift is dropped.
  return { name: PUNCTUATION[event.key] ?? event.key, shiftCounts: false };
}

export function chordOf(event: KeyboardEvent): string | null {
  const key = keyName(event);
  if (!key) return null;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("ctrl");
  if (event.altKey) parts.push("alt");
  if (event.shiftKey && key.shiftCounts) parts.push("shift");
  if (event.metaKey) parts.push("super");
  parts.push(key.name);
  return parts.join("+");
}

export type Keymap = {
  resolve: (context: BindingContext, event: KeyboardEvent) => KeyTarget | null;
  /** Display label of the first chord for a core action, for the help line. */
  label: (context: BindingContext, action: CoreAction) => string | null;
};

export function buildKeymap(bootstrap: Bootstrap): Keymap {
  const contexts = new Map(bootstrap.keymap.map((context) => [context.context, context]));
  const chain = (context: BindingContext) => {
    const entry = contexts.get(context);
    if (!entry) throw new Error(`The server sent no keymap for context ${context}`);
    return entry.fallbacks.map((fallback) => {
      const bindings = contexts.get(fallback)?.bindings;
      if (!bindings) throw new Error(`The server sent no keymap for context ${fallback}`);
      return bindings;
    });
  };
  return {
    resolve(context, event) {
      const chord = chordOf(event);
      if (!chord) return null;
      for (const bindings of chain(context)) {
        const binding = bindings.find((candidate) => candidate.chord === chord);
        if (binding) return binding.target;
      }
      return null;
    },
    label(context, action) {
      const seen = new Set<string>();
      for (const bindings of chain(context)) {
        for (const binding of bindings) {
          if (seen.has(binding.chord)) continue;
          seen.add(binding.chord);
          if (binding.target.kind === "core" && binding.target.action === action) {
            return binding.label;
          }
        }
      }
      return null;
    },
  };
}

export function coreAction(target: KeyTarget | null): CoreAction | null {
  return target?.kind === "core" ? target.action : null;
}
