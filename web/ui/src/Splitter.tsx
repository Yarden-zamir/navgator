import { useState } from "react";

/// A fraction in 0..1 that survives a reload.
export function useStoredFraction(key: string, initial: number): [number, (value: number) => void] {
  const [value, setValue] = useState(() => {
    const stored = Number(window.localStorage.getItem(key));
    return stored > 0 && stored < 1 ? stored : initial;
  });
  const store = (next: number) => {
    setValue(next);
    window.localStorage.setItem(key, String(next));
  };
  return [value, store];
}

const MIN = 0.2;
const MAX = 0.8;

/// A drag handle between two grid cells. It reports the pointer position as
/// a fraction of its parent. A double click restores `initial`.
export function Splitter(props: {
  direction: "columns" | "rows";
  initial: number;
  onChange: (fraction: number) => void;
}) {
  const [dragging, setDragging] = useState(false);
  return (
    <div
      className="splitter"
      data-direction={props.direction}
      data-dragging={dragging}
      role="separator"
      aria-orientation={props.direction === "columns" ? "vertical" : "horizontal"}
      title="Drag to resize, double-click to reset"
      onDoubleClick={() => props.onChange(props.initial)}
      onPointerDown={(event) => {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        setDragging(true);
      }}
      onPointerMove={(event) => {
        if (!dragging) return;
        const parent = event.currentTarget.parentElement;
        if (!parent) return;
        const rect = parent.getBoundingClientRect();
        const fraction =
          props.direction === "columns"
            ? (event.clientX - rect.left) / rect.width
            : (event.clientY - rect.top) / rect.height;
        props.onChange(Math.min(Math.max(fraction, MIN), MAX));
      }}
      onPointerUp={() => setDragging(false)}
      onPointerCancel={() => setDragging(false)}
    />
  );
}
