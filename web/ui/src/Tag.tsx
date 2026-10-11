import type { CSSProperties, MouseEvent } from "react";

/// The hue of a tag. It matches `tag_color` in `src/ui.rs` (FNV-1a over the
/// UTF-8 bytes, modulo 360), so a tag has the same color in both UIs.
export function tagHue(tag: string): number {
  let hash = 2166136261;
  for (const byte of new TextEncoder().encode(tag)) {
    hash = Math.imul(hash ^ byte, 16777619) >>> 0;
  }
  return hash % 360;
}

/// Add `#tag` to the search query, or remove it when the query has it.
export function toggleTagFilter(query: string, tag: string): string {
  const token = `#${tag}`;
  const words = query.split(" ").filter((word) => word !== "");
  const kept = words.filter((word) => word !== token);
  return (kept.length === words.length ? [...words, token] : kept).join(" ");
}

export function queryHasTag(query: string, tag: string): boolean {
  return query.split(" ").includes(`#${tag}`);
}

type TagProps = {
  name: string;
  /// The search query matches this tag.
  matched?: boolean;
  title?: string;
  onClick?: () => void;
  onRemove?: () => void;
};

export function Tag(props: TagProps) {
  const style = { "--tag-hue": tagHue(props.name) } as CSSProperties;
  const click = (handler: () => void) => (event: MouseEvent) => {
    // A tag sits inside a row or a panel that also reacts to clicks.
    event.stopPropagation();
    handler();
  };
  return (
    <span className="tag" style={style} data-matched={props.matched ?? false} title={props.title}>
      {props.onClick ? (
        <button type="button" className="tag-name" onClick={click(props.onClick)}>
          {props.name}
        </button>
      ) : (
        <span className="tag-name">{props.name}</span>
      )}
      {props.onRemove && (
        <button type="button" className="tag-remove" aria-label={`Remove ${props.name}`} onClick={click(props.onRemove)}>
          ×
        </button>
      )}
    </span>
  );
}
