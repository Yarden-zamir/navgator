import { useEffect, useRef, type RefObject } from "react";
import type { ResultRow } from "./api/ResultRow";
import type { SortMode } from "./api/SortMode";
import type { SortModeView } from "./api/SortModeView";
import { Tag, queryHasTag } from "./Tag";

type Props = {
  rows: ResultRow[];
  selectedIndex: number;
  query: string;
  title: string;
  status: { text: string; tone: "normal" | "busy" | "error" } | null;
  focused: boolean;
  inputRef: RefObject<HTMLInputElement | null>;
  sortModes: SortModeView[];
  sort: SortMode;
  remotes: boolean;
  onQuery: (query: string) => void;
  onFocus: () => void;
  onSelect: (index: number) => void;
  /// Open the action picker for the row.
  onActivate: (index: number) => void;
  onTagClick: (tag: string) => void;
  onSort: (sort: SortMode) => void;
  onToggleRemotes: () => void;
};

export function ResultsPanel(props: Props) {
  const listRef = useRef<HTMLOListElement>(null);

  useEffect(() => {
    const row = listRef.current?.children[props.selectedIndex];
    row?.scrollIntoView({ block: "nearest" });
  }, [props.selectedIndex, props.rows]);

  return (
    <section className="panel results" data-focused={props.focused} onMouseDown={props.onFocus}>
      <header className="panel-title">
        <span>{props.title}</span>
        {props.status && (
          <span className="status" data-tone={props.status.tone}>
            {props.status.text}
          </span>
        )}
      </header>
      <div className="search-box">
        <span className="search-icon" aria-hidden>
          {""}
        </span>
        <input
          ref={props.inputRef}
          className="search"
          value={props.query}
          placeholder="Search projects  ·  @folder  #tag"
          spellCheck={false}
          autoComplete="off"
          autoFocus
          onChange={(event) => props.onQuery(event.target.value)}
        />
        {props.query && (
          <button type="button" className="search-clear" title="Clear the search" onClick={() => props.onQuery("")}>
            ×
          </button>
        )}
      </div>
      <div className="results-toolbar">
        <label className="toolbar-field" title="Sort mode">
          <span aria-hidden>{""}</span>
          <select value={props.sort} onChange={(event) => props.onSort(event.target.value as SortMode)}>
            {props.sortModes.map((mode) => (
              <option key={mode.id} value={mode.id}>
                {mode.label}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="toggle-chip"
          aria-pressed={props.remotes}
          title="Show the remote branches of the selected repo"
          onClick={props.onToggleRemotes}
        >
          {""} Remote branches
        </button>
      </div>
      <ol className="rows" ref={listRef}>
        {props.rows.length === 0 && (
          <li className="empty">{props.query ? `Nothing matches "${props.query}"` : "No results"}</li>
        )}
        {props.rows.map((row, index) => (
          <li
            key={row.id}
            className="row"
            data-kind={row.kind}
            aria-selected={index === props.selectedIndex}
            title={row.metadataPath}
            onClick={() => props.onSelect(index)}
            onDoubleClick={() => props.onActivate(index)}
            onContextMenu={(event) => {
              event.preventDefault();
              props.onActivate(index);
            }}
          >
            <span className="row-name">
              {row.display.map((run, runIndex) =>
                run.hit ? <mark key={runIndex}>{run.text}</mark> : <span key={runIndex}>{run.text}</span>,
              )}
            </span>
            {row.context && <span className="row-context">{row.context}</span>}
            {!row.context && row.tags.length > 0 && (
              <span className="row-tags">
                {[...row.tags]
                  .sort((left, right) => Number(right.matched) - Number(left.matched))
                  .map((tag) => (
                    <Tag
                      key={tag.name}
                      name={tag.name}
                      matched={tag.matched}
                      title={queryHasTag(props.query, tag.name) ? `Stop filtering by #${tag.name}` : `Filter by #${tag.name}`}
                      onClick={() => props.onTagClick(tag.name)}
                    />
                  ))}
              </span>
            )}
            <span className="row-date">{row.date ?? ""}</span>
            <button
              type="button"
              className="row-more"
              title="Actions (or right-click the row)"
              onClick={(event) => {
                event.stopPropagation();
                props.onActivate(index);
              }}
            >
              {""}
            </button>
          </li>
        ))}
      </ol>
    </section>
  );
}
