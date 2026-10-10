import type { RefObject } from "react";
import type { GitSummary } from "./api/GitSummary";
import type { GithubView } from "./api/GithubView";
import type { Preview } from "./api/Preview";
import type { PreviewTabView } from "./api/PreviewTabView";
import type { TreeView } from "./api/TreeView";
import type { Loaded } from "./useCached";

export type TagEdit = { path: string; tags: string[]; input: string; suggestions: string[] };

type PreviewProps = {
  title: string;
  preview: Loaded<Preview> | null;
  tabs: PreviewTabView[];
  activeTabIndex: number;
  tree: Loaded<TreeView> | null;
  tags: string[];
  tagEdit: TagEdit | null;
  tagInputRef: RefObject<HTMLInputElement | null>;
  focused: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
  onFocus: () => void;
  onSelectTab: (index: number) => void;
  onTagInput: (value: string) => void;
};

export function PreviewPanel(props: PreviewProps) {
  return (
    <section className="panel preview" data-focused={props.focused} onMouseDown={props.onFocus}>
      <header className="panel-title">
        <span>{props.title}</span>
      </header>
      {props.tabs.length > 1 && (
        <nav className="tabs">
          {props.tabs.map((tab, index) => (
            <button
              key={tab.path}
              type="button"
              className="tab"
              aria-selected={index === props.activeTabIndex}
              title={tab.path}
              onClick={() => props.onSelectTab(index)}
            >
              {tab.label}
            </button>
          ))}
        </nav>
      )}
      <div className="panel-body" ref={props.scrollRef}>
        {(props.tags.length > 0 || props.tagEdit) && (
          <div className="tag-line">
            {(props.tagEdit?.tags ?? props.tags).map((tag) => (
              <span key={tag} className="tag">
                {tag}
              </span>
            ))}
            {props.tagEdit && (
              <>
                <input
                  ref={props.tagInputRef}
                  className="tag-input"
                  value={props.tagEdit.input}
                  placeholder="new tag"
                  list="tag-suggestions"
                  spellCheck={false}
                  autoFocus
                  onChange={(event) => props.onTagInput(event.target.value)}
                />
                <datalist id="tag-suggestions">
                  {props.tagEdit.suggestions.map((tag) => (
                    <option key={tag} value={tag} />
                  ))}
                </datalist>
              </>
            )}
          </div>
        )}
        <PreviewBody preview={props.preview} tree={props.tree} />
      </div>
    </section>
  );
}

function PreviewBody({ preview, tree }: { preview: Loaded<Preview> | null; tree: Loaded<TreeView> | null }) {
  if (preview === null) return <p className="muted">No selection</p>;
  if (preview.state === "loading") return <p className="loading">Loading preview...</p>;
  if (preview.state === "failed") return <p className="error-text">{preview.error}</p>;
  const data = preview.value;
  if (data.kind === "remote") {
    return (
      <div className="sections">
        <Section title="Remote branch" lines={[data.remoteBranch]} />
        <Section title="Target worktree" lines={[data.targetDisplay]} />
        {data.recentCommits ? (
          <Section title="Recent commits" lines={data.recentCommits} />
        ) : (
          <p className="muted">No git summary available</p>
        )}
      </div>
    );
  }
  if (tree === null || tree.state === "loading") return <p className="loading">Loading preview...</p>;
  if (tree.state === "failed") return <p className="error-text">{tree.error}</p>;
  const view = tree.value;
  return (
    <div className="sections">
      <p className="path">{view.displayPath}</p>
      {!view.isDir ? (
        <p className="muted">Not a directory</p>
      ) : view.lines === null ? (
        <p className="muted">erd output not available</p>
      ) : (
        <section>
          <h3>Contents</h3>
          <pre className="tree">
            {view.lines.map((line, lineIndex) => (
              <div key={lineIndex}>
                {line.map((segment, index) => (
                  <span
                    key={index}
                    style={segment.color ? { color: segment.color } : undefined}
                    className={segment.bold ? "bold" : undefined}
                  >
                    {segment.text}
                  </span>
                ))}
                {line.length === 0 && "\n"}
              </div>
            ))}
          </pre>
        </section>
      )}
    </div>
  );
}

function Section({ title, lines }: { title: string; lines: string[] }) {
  if (lines.length === 0) return null;
  return (
    <section>
      <h3>{title}</h3>
      <pre>{lines.join("\n")}</pre>
    </section>
  );
}

export type DetailTab = { label: "GitHub"; github: GithubView } | { label: "Git"; git: GitSummary };

type DetailProps = {
  tabs: DetailTab[];
  activeIndex: number;
  focused: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
  onFocus: () => void;
  onSelectTab: (index: number) => void;
};

export function DetailPanel(props: DetailProps) {
  const active = props.tabs[props.activeIndex] ?? props.tabs[0];
  if (!active) return null;
  return (
    <section className="panel detail" data-focused={props.focused} onMouseDown={props.onFocus}>
      {props.tabs.length > 1 ? (
        <nav className="tabs">
          {props.tabs.map((tab, index) => (
            <button
              key={tab.label}
              type="button"
              className="tab"
              aria-selected={tab === active}
              onClick={() => props.onSelectTab(index)}
            >
              {tab.label}
            </button>
          ))}
        </nav>
      ) : (
        <header className="panel-title">
          <span>{active.label}</span>
        </header>
      )}
      <div className="panel-body" ref={props.scrollRef}>
        {"github" in active ? <GithubBody view={active.github} /> : <GitBody git={active.git} />}
      </div>
    </section>
  );
}

function GithubBody({ view }: { view: GithubView }) {
  const summary = view.summary;
  const facts = summary
    ? [
        summary.language && `Language: ${summary.language}`,
        summary.defaultBranch && `Default branch: ${summary.defaultBranch}`,
        summary.stars !== null && `Stars: ${summary.stars}`,
        summary.forks !== null && `Forks: ${summary.forks}`,
        summary.openIssues !== null && `Open issues: ${summary.openIssues}`,
      ].filter((fact): fact is string => typeof fact === "string")
    : [];
  return (
    <div className="sections">
      {summary?.description && <p className="description">{summary.description}</p>}
      {facts.length > 0 && <p className="muted">{facts.join(" | ")}</p>}
      {view.readmeHtml && (
        // The server renders the README without raw HTML and with safe links only.
        <article className="readme" dangerouslySetInnerHTML={{ __html: view.readmeHtml }} />
      )}
    </div>
  );
}

function GitBody({ git }: { git: GitSummary }) {
  return (
    <div className="sections">
      {git.bareRepository && <h3>Bare repository</h3>}
      {git.branch && <h3>Branch: {git.branch}</h3>}
      {git.worktrees && (
        <section>
          <h3>Worktrees</h3>
          {git.worktrees.length === 0 ? (
            <pre>No worktrees</pre>
          ) : (
            <table className="worktrees">
              <tbody>
                {git.worktrees.map((worktree) => (
                  <tr key={worktree.path}>
                    <td>{worktree.label}</td>
                    <td className="muted">{worktree.path}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>
      )}
      <Section title="Recent commits" lines={git.recentCommits} />
      <Section title="Staged changes" lines={git.staged} />
      <Section title="Unstaged changes" lines={git.unstaged} />
      <Section title="Untracked" lines={git.untracked} />
    </div>
  );
}
