import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, runCreate, streamProgress } from "./api";
import type { ActionRef } from "./api/ActionRef";
import type { BindingContext } from "./api/BindingContext";
import type { Bootstrap } from "./api/Bootstrap";
import type { CoreAction } from "./api/CoreAction";
import type { Preview } from "./api/Preview";
import type { PreviewTabView } from "./api/PreviewTabView";
import type { ResultRow } from "./api/ResultRow";
import type { Results } from "./api/Results";
import type { SortMode } from "./api/SortMode";
import { buildKeymap, coreAction } from "./keys";
import { ActionPicker, CreateForm, CreatePicker, ProgressOverlay, type CreateFormState } from "./Overlays";
import { DetailPanel, PreviewPanel, type DetailTab, type TagEdit } from "./PreviewPanel";
import { toggleTagFilter } from "./Tag";
import { ResultsPanel } from "./ResultsPanel";
import { Cache, useCached } from "./useCached";

type Focus = "search" | "preview" | "detail" | "tag-edit";

/** What Enter (or a configured binding) does once the target path is known. */
type TargetAction = { kind: "actions" } | { kind: "configured"; id: string };

type Overlay =
  | { kind: "actions"; path: string; query: string; selected: number; visible: number[] }
  | { kind: "create-picker"; path: string | null; query: string; selected: number }
  | { kind: "create-form"; form: CreateFormState }
  | { kind: "progress"; title: string; messages: string[]; error: string | null };

const FOCUS_CONTEXT: Record<Focus, BindingContext> = {
  search: "navigator",
  preview: "preview",
  detail: "detail",
  "tag-edit": "tag-editor",
};

const SCROLL_STEP = 24;

function isTypedCharacter(event: KeyboardEvent): boolean {
  return [...event.key].length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey;
}

function wrap(index: number, length: number): number {
  return length === 0 ? 0 : (index + length) % length;
}

/** Preview tabs whose label or path contains the filter; all tabs when none match. */
function visibleTabIndexes(tabs: PreviewTabView[], filter: string): number[] {
  const needle = filter.trim().toLowerCase();
  const all = tabs.map((_, index) => index);
  if (!needle) return all;
  const matches = all.filter((index) => {
    const tab = tabs[index];
    return tab !== undefined && (tab.label.toLowerCase().includes(needle) || tab.path.toLowerCase().includes(needle));
  });
  return matches.length > 0 ? matches : all;
}

function filterCreates(bootstrap: Bootstrap, query: string) {
  const needle = query.trim().toLowerCase();
  return bootstrap.creates.filter(
    (item) =>
      !needle ||
      item.label.toLowerCase().includes(needle) ||
      (item.icon ?? "").toLowerCase().includes(needle),
  );
}

export function App({ bootstrap }: { bootstrap: Bootstrap }) {
  const keymap = useMemo(() => buildKeymap(bootstrap), [bootstrap]);
  const caches = useMemo(
    () => ({
      preview: new Cache<Preview>((entry) => api.preview(entry)),
      tree: new Cache((path) => api.tree(path)),
      git: new Cache((key) => {
        const [bare, path] = [key.slice(0, 1) === "1", key.slice(2)];
        return api.git(path, bare);
      }),
      github: new Cache((path) => api.github(path)),
    }),
    [],
  );

  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortMode>(bootstrap.defaultSort);
  const [remotes, setRemotes] = useState(bootstrap.remotesByDefault);
  const [results, setResults] = useState<Results | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [stickToFirst, setStickToFirst] = useState(true);
  const [focus, setFocus] = useState<Focus>("search");
  const [tabVisibleIndex, setTabVisibleIndex] = useState(0);
  const [worktreeFilter, setWorktreeFilter] = useState("");
  const [detailIndex, setDetailIndex] = useState(0);
  const [tagEdit, setTagEdit] = useState<TagEdit | null>(null);
  const [overlay, setOverlay] = useState<Overlay | null>(null);
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [remoteNote, setRemoteNote] = useState<string | null>(null);

  const searchRef = useRef<HTMLInputElement>(null);
  const tagInputRef = useRef<HTMLInputElement>(null);
  const previewScrollRef = useRef<HTMLDivElement>(null);
  const detailScrollRef = useRef<HTMLDivElement>(null);
  const lastIndexRef = useRef(0);

  // ------------------------------------------------------------- results

  const resultsRequest = useRef(0);
  const loadResults = useCallback(async () => {
    const request = ++resultsRequest.current;
    const loaded = await api.results(query, sort, remotes);
    if (request === resultsRequest.current) setResults(loaded);
  }, [query, sort, remotes]);
  const loadResultsRef = useRef(loadResults);
  loadResultsRef.current = loadResults;

  const showError = useCallback((error: unknown) => {
    setNotice({ text: error instanceof Error ? error.message : String(error), error: true });
  }, []);

  useEffect(() => {
    loadResults().catch(showError);
  }, [loadResults, showError]);

  // Every server-side change (worktree scans, dates, tags, remotes) pings
  // this stream; refetch at most every 150 ms while changes flood in.
  useEffect(() => {
    const events = new EventSource("/api/events");
    let timer: number | null = null;
    events.addEventListener("change", () => {
      if (timer !== null) return;
      timer = window.setTimeout(() => {
        timer = null;
        loadResultsRef.current().catch(showError);
      }, 150);
    });
    return () => {
      events.close();
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [showError]);

  const rows = results?.rows ?? [];
  const selectedIndex = useMemo(() => {
    if (rows.length === 0) return 0;
    if (stickToFirst) return 0;
    const found = rows.findIndex((row) => row.id === selectedId);
    return found >= 0 ? found : Math.min(lastIndexRef.current, rows.length - 1);
  }, [rows, selectedId, stickToFirst]);
  lastIndexRef.current = selectedIndex;
  const entry: ResultRow | null = rows[selectedIndex] ?? null;

  // Remote branches load for the first result when they are on by default.
  const remotesStarted = useRef(false);
  useEffect(() => {
    if (remotesStarted.current || !bootstrap.remotesByDefault || !rows[0]) return;
    remotesStarted.current = true;
    api.loadRemotes(rows[0].id).catch(showError);
  }, [bootstrap.remotesByDefault, rows, showError]);

  const selectIndex = (index: number) => {
    const row = rows[index];
    if (!row) return;
    setStickToFirst(false);
    setSelectedId(row.id);
  };

  // ------------------------------------------------------------- preview

  const preview = useCached(caches.preview, entry?.id ?? null);
  const previewData = preview?.state === "ready" ? preview.value : null;
  const tabs: PreviewTabView[] = useMemo(() => {
    if (!previewData) return [];
    if (previewData.kind === "remote") {
      return [{ path: previewData.targetPath, label: previewData.remoteBranch }];
    }
    return previewData.tabs;
  }, [previewData]);
  const visibleTabs = useMemo(() => visibleTabIndexes(tabs, worktreeFilter), [tabs, worktreeFilter]);
  const activeTabIndex = visibleTabs[Math.min(tabVisibleIndex, visibleTabs.length - 1)] ?? 0;
  const activeTab = previewData?.kind === "local" ? tabs[activeTabIndex] ?? null : null;

  // A new result starts on its preferred tab, like the terminal UI.
  useEffect(() => {
    setWorktreeFilter("");
    setDetailIndex(0);
    if (previewData?.kind !== "local") {
      setTabVisibleIndex(0);
      return;
    }
    const preferred = previewData.preferredPath;
    const index = preferred ? previewData.tabs.findIndex((tab) => tab.path === preferred) : -1;
    setTabVisibleIndex(Math.max(index, 0));
  }, [previewData]);

  useEffect(() => {
    previewScrollRef.current?.scrollTo({ top: 0 });
    detailScrollRef.current?.scrollTo({ top: 0 });
  }, [activeTab?.path, entry?.id]);

  const tree = useCached(caches.tree, activeTab?.path ?? null);
  const bare = previewData?.kind === "local" && previewData.selectedRepoIsBare;
  const git = useCached(caches.git, activeTab ? `${bare ? 1 : 0}:${activeTab.path}` : null);
  const github = useCached(caches.github, activeTab?.path ?? null);
  const detailTabs: DetailTab[] = [];
  if (github?.state === "ready" && github.value) detailTabs.push({ label: "GitHub", github: github.value });
  if (git?.state === "ready" && git.value) detailTabs.push({ label: "Git", git: git.value });
  const effectiveFocus: Focus = focus === "detail" && detailTabs.length === 0 ? "preview" : focus;

  const overlayOpen = overlay !== null;
  useEffect(() => {
    if (overlayOpen) return;
    if (effectiveFocus === "search") searchRef.current?.focus();
    else if (effectiveFocus !== "tag-edit") (document.activeElement as HTMLElement | null)?.blur();
  }, [effectiveFocus, overlayOpen]);

  // ------------------------------------------------------------- actions

  const actionsFor = useCallback(
    async (path: string, actionQuery: string) => api.actions(path, actionQuery),
    [],
  );

  const targetPath = async (): Promise<string | null> => {
    if (!entry) return null;
    const tab = effectiveFocus === "preview" || effectiveFocus === "detail" ? activeTab?.path : undefined;
    return (await api.target(entry.id, tab)).path;
  };

  const progress = (title: string, first: string) => {
    setOverlay({ kind: "progress", title, messages: [first], error: null });
    return {
      message: (text: string) =>
        setOverlay((current) =>
          current?.kind === "progress" ? { ...current, messages: [...current.messages, text] } : current,
        ),
      fail: (error: unknown) =>
        setOverlay((current) =>
          current?.kind === "progress"
            ? { ...current, error: error instanceof Error ? error.message : String(error) }
            : { kind: "progress", title, messages: [], error: String(error) },
        ),
    };
  };

  const openActions = async (path: string) => {
    const visible = await actionsFor(path, "");
    setOverlay({ kind: "actions", path, query: "", selected: 0, visible });
  };

  const runAction = async (ref: ActionRef, path: string, label: string) => {
    const status = progress(`Running ${label}`, path);
    try {
      const result = await api.runAction(ref, path);
      setOverlay(null);
      setNotice({
        text: result.stillRunning ? `${label} is still running` : result.output ?? `Ran ${label}`,
        error: false,
      });
    } catch (error) {
      status.fail(error);
    }
  };

  const dispatchTarget = async (target: TargetAction, path: string) => {
    if (target.kind === "actions") return openActions(path);
    const action = bootstrap.actions.find((candidate) => candidate.id === target.id);
    return runAction({ by: "id", id: target.id }, path, action?.label ?? target.id);
  };

  /** Enter, the actions key and configured bindings: resolve the path, then act. */
  const startTarget = async (target: TargetAction) => {
    if (!entry) return;
    setStickToFirst(false);
    setSelectedId(entry.id);
    if (entry.kind === "remote-branch") {
      const verb = bootstrap.branchOnSelect === "worktree" ? "Creating" : "Checking out";
      const status = progress(`${verb} ${entry.display.map((run) => run.text).join("")}`, "Starting");
      try {
        const path = await streamProgress("/api/branches/select", { entryId: entry.id }, status.message);
        caches.preview.clear();
        setOverlay(null);
        await dispatchTarget(target, path);
      } catch (error) {
        status.fail(error);
      }
      return;
    }
    const path = await targetPath();
    if (path) await dispatchTarget(target, path);
  };

  const deleteWorktree = async () => {
    if (entry?.kind !== "worktree") return;
    setStickToFirst(false);
    const status = progress(
      `Delete ${entry.display.map((run) => run.text).join("")}`,
      "Starting safety checks",
    );
    try {
      await streamProgress("/api/worktrees/delete", { entryId: entry.id }, status.message);
      caches.preview.clear();
      setOverlay(null);
    } catch (error) {
      status.fail(error);
    }
  };

  const toggleRemotes = async () => {
    setStickToFirst(false);
    const next = !remotes;
    setRemotes(next);
    setRemoteNote(null);
    if (!next) return;
    if (!entry) {
      setRemoteNote("No repo selected");
      return;
    }
    if (entry) setSelectedId(entry.id);
    await api.loadRemotes(entry.id);
  };

  const editTags = async () => {
    if (!entry) return;
    setStickToFirst(false);
    const suggestions = await api.tagSuggestions();
    setTagEdit({ path: entry.metadataPath, tags: entry.tags.map((tag) => tag.name), input: "", suggestions });
    setFocus("tag-edit");
  };

  const cycleSort = () => {
    const modes = bootstrap.sortModes.map((mode) => mode.id);
    setSort(modes[wrap(modes.indexOf(sort) + 1, modes.length)] ?? sort);
    setStickToFirst(true);
  };

  const copyPath = async () => {
    const path = await targetPath();
    if (!path) return;
    await navigator.clipboard.writeText(path);
    setNotice({ text: `Copied ${path}`, error: false });
  };

  const openCreate = async () => {
    if (bootstrap.creates.length === 0) return;
    setStickToFirst(false);
    setOverlay({ kind: "create-picker", path: await targetPath(), query: "", selected: 0 });
  };

  // ------------------------------------------------------------- create

  const createItem = (form: CreateFormState) => {
    const item = bootstrap.creates[form.index];
    if (!item) throw new Error(`Unknown create recipe ${form.index}`);
    return item;
  };

  const withSuggestions = async (form: CreateFormState): Promise<CreateFormState> => {
    const prompt = createItem(form).prompts[form.active];
    if (!prompt?.path) return { ...form, suggestions: [], suggestion: 0, completions: false };
    const suggestions = await api.pathCompletions(form.values[form.active] ?? "");
    return {
      ...form,
      suggestions,
      suggestion: Math.min(form.suggestion, Math.max(suggestions.length - 1, 0)),
      completions: form.completions && suggestions.length > 0,
    };
  };

  const formRequest = useRef(0);
  const updateForm = async (next: CreateFormState, refill: boolean) => {
    const request = ++formRequest.current;
    setOverlay({ kind: "create-form", form: next });
    const values = refill ? await api.createValues(next.index, next.path, next.values, next.active) : next.values;
    const form = await withSuggestions({ ...next, values });
    if (request === formRequest.current) setOverlay({ kind: "create-form", form });
  };

  const openCreateForm = async (index: number, path: string | null) => {
    const item = bootstrap.creates[index];
    if (!item) return;
    if (item.prompts.length === 0) return startCreate(index, path, []);
    const values = await api.createValues(index, path, null);
    const form = await withSuggestions({
      index,
      path,
      values,
      active: 0,
      error: null,
      suggestions: [],
      suggestion: 0,
      completions: false,
    });
    setOverlay({ kind: "create-form", form });
  };

  const startCreate = async (index: number, path: string | null, values: string[], form?: CreateFormState) => {
    const item = bootstrap.creates[index];
    if (!item) return;
    const status = progress(`Creating ${item.label}`, "Starting create recipe");
    try {
      const outcome = await runCreate({ index, path, values }, status.message);
      if ("missing" in outcome) {
        if (!form) throw new Error(outcome.missing.message);
        await updateForm({ ...form, active: outcome.missing.promptIndex, error: outcome.missing.message }, false);
        return;
      }
      caches.preview.clear();
      setNotice({ text: `Created ${outcome.path}`, error: false });
      // The terminal UI changes into the new path; the web app offers its actions.
      await openActions(outcome.path);
    } catch (error) {
      status.fail(error);
    }
  };

  // ------------------------------------------------------------- tags

  const commitTagInput = (edit: TagEdit): TagEdit => {
    const raw = edit.input.trim();
    if (!raw) return edit;
    const lower = raw.toLowerCase();
    const chosen = edit.suggestions.find((tag) => tag.toLowerCase().startsWith(lower)) ?? raw;
    return { ...edit, input: "", tags: edit.tags.includes(chosen) ? edit.tags : [...edit.tags, chosen] };
  };

  const cancelTags = () => {
    setTagEdit(null);
    setFocus("preview");
  };

  const filterByTag = (tag: string) => {
    setQuery((current) => toggleTagFilter(current, tag));
    setStickToFirst(true);
    setFocus("search");
  };

  const confirmTags = async (edit: TagEdit) => {
    const committed = commitTagInput(edit);
    await api.saveTags(committed.path, committed.tags);
    setTagEdit(null);
    setFocus("preview");
  };

  // ------------------------------------------------------------- keys

  const scroll = (element: HTMLDivElement | null, action: CoreAction) => {
    if (!element) return;
    const page = element.clientHeight * 0.9;
    const by: Partial<Record<CoreAction, number>> = {
      "move-up": -SCROLL_STEP,
      "move-down": SCROLL_STEP,
      "page-up": -page,
      "page-down": page,
      "move-home": -element.scrollHeight,
      "move-end": element.scrollHeight,
    };
    const top = by[action];
    if (top !== undefined) element.scrollBy({ top });
  };
  const atTop = (element: HTMLDivElement | null) => !element || element.scrollTop <= 0;
  const atBottom = (element: HTMLDivElement | null) =>
    !element || element.scrollTop + element.clientHeight >= element.scrollHeight - 1;

  const moveTab = (delta: number): boolean => {
    const next = tabVisibleIndex + delta;
    if (next < 0 || next >= visibleTabs.length) return false;
    setTabVisibleIndex(next);
    setDetailIndex(0);
    return true;
  };

  const handleOverlayKey = (event: KeyboardEvent, current: Overlay): void => {
    if (current.kind === "progress") {
      const target = keymap.resolve(current.error === null ? "progress-overlay" : "error-overlay", event);
      if (!target) return;
      event.preventDefault();
      const action = coreAction(target);
      // A running operation cannot stop; only a finished error closes.
      if (current.error !== null && (action === "back" || action === "dismiss-overlay" || action === "cancel")) {
        setOverlay(null);
      }
      return;
    }

    if (current.kind === "actions") {
      const target = keymap.resolve("action-picker", event);
      const action = coreAction(target);
      if (target) event.preventDefault();
      const updateQuery = (actionQuery: string) => {
        setOverlay({ ...current, query: actionQuery });
        actionsFor(current.path, actionQuery)
          .then((visible) =>
            setOverlay((latest) =>
              latest?.kind === "actions" && latest.query === actionQuery
                ? { ...latest, visible, selected: Math.min(latest.selected, Math.max(visible.length - 1, 0)) }
                : latest,
            ),
          )
          .catch(showError);
      };
      switch (action) {
        case "cancel":
          setOverlay(null);
          return;
        case "move-up":
          setOverlay({ ...current, selected: wrap(current.selected - 1, current.visible.length) });
          return;
        case "move-down":
          setOverlay({ ...current, selected: wrap(current.selected + 1, current.visible.length) });
          return;
        case "run":
        case "run-and-close": {
          // There is no session to close in a browser, so both keys run.
          const index = current.visible[current.selected];
          const view = index === undefined ? undefined : bootstrap.actions[index];
          if (view) runAction({ by: "index", index: view.index }, current.path, view.label).catch(showError);
          return;
        }
      }
      if (target) return;
      if (event.key === "Backspace") {
        event.preventDefault();
        updateQuery(current.query.slice(0, -1));
      } else if (isTypedCharacter(event)) {
        event.preventDefault();
        updateQuery(current.query + event.key);
      }
      return;
    }

    if (current.kind === "create-picker") {
      const target = keymap.resolve("create-picker", event);
      const action = coreAction(target);
      if (target) event.preventDefault();
      const visible = filterCreates(bootstrap, current.query);
      switch (action) {
        case "cancel":
          setOverlay(null);
          return;
        case "move-up":
          setOverlay({ ...current, selected: wrap(current.selected - 1, visible.length) });
          return;
        case "move-down":
          setOverlay({ ...current, selected: wrap(current.selected + 1, visible.length) });
          return;
        case "confirm": {
          const item = visible[current.selected];
          if (item) openCreateForm(item.index, current.path).catch(showError);
          return;
        }
      }
      if (target) return;
      const clamp = (createQuery: string) =>
        Math.min(current.selected, Math.max(filterCreates(bootstrap, createQuery).length - 1, 0));
      if (event.key === "Backspace") {
        event.preventDefault();
        const createQuery = current.query.slice(0, -1);
        setOverlay({ ...current, query: createQuery, selected: clamp(createQuery) });
      } else if (isTypedCharacter(event)) {
        event.preventDefault();
        const createQuery = current.query + event.key;
        setOverlay({ ...current, query: createQuery, selected: clamp(createQuery) });
      }
      return;
    }

    const form = current.form;
    const item = createItem(form);
    const target = keymap.resolve(form.completions ? "create-completions" : "create-form", event);
    const action = coreAction(target);
    if (target) event.preventDefault();
    switch (action) {
      case "back":
        setOverlay({ kind: "create-picker", path: form.path, query: "", selected: form.index });
        return;
      case "cancel":
        setOverlay(null);
        return;
      case "confirm":
        startCreate(form.index, form.path, form.values, form).catch(showError);
        return;
      case "accept": {
        const suggestion = form.suggestions[form.suggestion];
        if (!suggestion) return;
        const values = form.values.map((value, index) => (index === form.active ? suggestion.value : value));
        updateForm({ ...form, values, completions: false }, false).catch(showError);
        return;
      }
      case "move-right":
        if (form.suggestions.length > 0) setOverlay({ kind: "create-form", form: { ...form, completions: true } });
        return;
      case "move-left":
        setOverlay({ kind: "create-form", form: { ...form, completions: false } });
        return;
      case "move-up":
      case "move-down": {
        const delta = action === "move-up" ? -1 : 1;
        if (form.completions) {
          setOverlay({
            kind: "create-form",
            form: { ...form, suggestion: wrap(form.suggestion + delta, form.suggestions.length) },
          });
        } else {
          updateForm(
            { ...form, active: wrap(form.active + delta, item.prompts.length), error: null, completions: false },
            false,
          ).catch(showError);
        }
        return;
      }
      case "clear-input": {
        const values = form.values.map((value, index) => (index === form.active ? "" : value));
        updateForm({ ...form, values, error: null }, true).catch(showError);
        return;
      }
    }
    // Unbound keys type into the active input.
  };

  const handleKey = (event: KeyboardEvent): void => {
    if (event.isComposing) return;
    if (overlay) return handleOverlayKey(event, overlay);

    const context = FOCUS_CONTEXT[effectiveFocus];
    const target = keymap.resolve(context, event);
    if (target?.kind === "disabled") {
      event.preventDefault();
      return;
    }
    const action = coreAction(target);

    if (effectiveFocus === "tag-edit" && tagEdit) {
      switch (action) {
        case "confirm":
          event.preventDefault();
          confirmTags(tagEdit).catch(showError);
          return;
        case "accept":
          event.preventDefault();
          setTagEdit(commitTagInput(tagEdit));
          return;
        case "remove-last-tag":
          // With text in the input, Backspace edits the text instead.
          if (tagEdit.input === "") {
            event.preventDefault();
            setTagEdit({ ...tagEdit, tags: tagEdit.tags.slice(0, -1) });
          }
          return;
        case "clear-input":
          event.preventDefault();
          setTagEdit({ ...tagEdit, input: "" });
          return;
        case "cancel":
          event.preventDefault();
          cancelTags();
          return;
        case "copy-path":
          event.preventDefault();
          copyPath().catch(showError);
          return;
        case "cycle-sort":
          event.preventDefault();
          cycleSort();
          return;
      }
      return;
    }

    const shared: Partial<Record<CoreAction, () => Promise<void> | void>> = {
      navigate: () => startTarget({ kind: "actions" }),
      actions: () => startTarget({ kind: "actions" }),
      create: openCreate,
      "copy-path": copyPath,
      "delete-worktree": deleteWorktree,
      "toggle-remotes": toggleRemotes,
      "edit-tags": editTags,
      "cycle-sort": cycleSort,
    };
    const sharedHandler = action ? shared[action] : undefined;
    if (sharedHandler) {
      event.preventDefault();
      Promise.resolve(sharedHandler()).catch(showError);
      return;
    }
    if (target?.kind === "configured") {
      event.preventDefault();
      startTarget({ kind: "configured", id: target.id }).catch(showError);
      return;
    }
    if (action === "cancel") {
      event.preventDefault();
      if (effectiveFocus !== "search") setFocus("search");
      else {
        setQuery("");
        setStickToFirst(true);
      }
      return;
    }

    if (effectiveFocus === "search") {
      const input = searchRef.current;
      switch (action) {
        case "move-up":
          event.preventDefault();
          selectIndex(Math.max(selectedIndex - 1, 0));
          return;
        case "move-down":
          event.preventDefault();
          selectIndex(Math.min(selectedIndex + 1, rows.length - 1));
          return;
        case "move-right":
          if (input && input.selectionStart === input.value.length) {
            event.preventDefault();
            setStickToFirst(false);
            if (entry) setSelectedId(entry.id);
            setFocus("preview");
          }
          return;
        case "move-home":
          event.preventDefault();
          input?.setSelectionRange(0, 0);
          return;
        case "move-end":
          event.preventDefault();
          input?.setSelectionRange(input.value.length, input.value.length);
          return;
        case "clear-input":
          event.preventDefault();
          setQuery("");
          setStickToFirst(true);
          return;
      }
      return;
    }

    if (effectiveFocus === "preview") {
      const element = previewScrollRef.current;
      if (action) event.preventDefault();
      switch (action) {
        case "clear-input":
          if (worktreeFilter) {
            setWorktreeFilter("");
            setTabVisibleIndex(0);
          }
          return;
        case "move-left":
          if (!moveTab(-1)) setFocus("search");
          return;
        case "move-right":
          if (!moveTab(1) && detailTabs.length > 0) setFocus("detail");
          return;
        case "move-up":
          if (!atTop(element)) scroll(element, action);
          else if (!moveTab(-1)) setFocus("search");
          return;
        case "move-down":
          if (!atBottom(element)) scroll(element, action);
          else if (!moveTab(1) && detailTabs.length > 0) setFocus("detail");
          return;
        case "page-up":
        case "page-down":
        case "move-home":
        case "move-end":
          scroll(element, action);
          return;
      }
      // Typing filters the worktree tabs by label or path.
      if (event.key === "Backspace" && worktreeFilter) {
        event.preventDefault();
        setWorktreeFilter(worktreeFilter.slice(0, -1));
        setTabVisibleIndex(0);
      } else if (isTypedCharacter(event)) {
        event.preventDefault();
        setWorktreeFilter(worktreeFilter + event.key);
        setTabVisibleIndex(0);
      }
      return;
    }

    const element = detailScrollRef.current;
    if (action) event.preventDefault();
    switch (action) {
      case "move-left":
        if (detailIndex > 0) setDetailIndex(detailIndex - 1);
        else setFocus("preview");
        return;
      case "move-right":
        if (detailIndex + 1 < detailTabs.length) setDetailIndex(detailIndex + 1);
        else setFocus("preview");
        return;
      case "move-up":
        if (!atTop(element)) scroll(element, action);
        else setFocus("preview");
        return;
      case "move-down":
      case "page-up":
      case "page-down":
      case "move-home":
      case "move-end":
        scroll(element, action);
        return;
    }
  };

  const handleKeyRef = useRef(handleKey);
  handleKeyRef.current = handleKey;
  useEffect(() => {
    const listener = (event: KeyboardEvent) => handleKeyRef.current(event);
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), notice.error ? 8000 : 3000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  // ------------------------------------------------------------- render

  const sortLabel = bootstrap.sortModes.find((mode) => mode.id === sort)?.label ?? sort;
  const remoteStatus = remoteNote ?? results?.remote.message ?? null;
  const resultsTitle = `Results ${rows.length}/${results?.total ?? 0}${remotes ? " + remote" : ""}`;
  const status = remotes
    ? remoteStatus
      ? {
          text: remoteStatus,
          tone: remoteNote || results?.remote.error ? ("error" as const) : results?.remote.fetching ? ("busy" as const) : ("normal" as const),
        }
      : null
    : null;

  const helpContext: BindingContext = overlay
    ? overlay.kind === "actions"
      ? "action-picker"
      : overlay.kind === "create-picker"
        ? "create-picker"
        : overlay.kind === "create-form"
          ? overlay.form.completions
            ? "create-completions"
            : "create-form"
          : overlay.error === null
            ? "progress-overlay"
            : "error-overlay"
    : FOCUS_CONTEXT[effectiveFocus];
  const hint = (action: CoreAction, text: string): ReactNode => {
    const label = keymap.label(helpContext, action);
    return label ? (
      <span className="hint" key={action}>
        <kbd>{label}</kbd> {text}
      </span>
    ) : null;
  };
  const help: ReactNode[] =
    helpContext === "navigator" || helpContext === "preview" || helpContext === "detail"
      ? [
          hint("navigate", "actions"),
          hint("actions", "actions"),
          hint("create", "create"),
          hint("copy-path", "copy path"),
          entry?.kind === "worktree" ? hint("delete-worktree", "delete worktree") : null,
          hint("toggle-remotes", remotes ? "hide remotes" : "remotes"),
          hint("edit-tags", "tags"),
          hint("cycle-sort", `sort: ${sortLabel}`),
          helpContext === "navigator" ? hint("move-right", "preview") : hint("move-left", "back"),
        ]
      : helpContext === "tag-editor"
        ? [hint("confirm", "save"), hint("accept", "add tag"), hint("remove-last-tag", "remove last"), hint("cancel", "cancel")]
        : helpContext === "action-picker"
          ? [hint("run", "run"), hint("move-down", "next"), hint("cancel", "close")]
          : helpContext === "create-picker"
            ? [hint("confirm", "choose"), hint("move-down", "next"), hint("cancel", "close")]
            : helpContext === "create-form" || helpContext === "create-completions"
              ? [
                  hint("confirm", "create"),
                  hint("move-down", "next field"),
                  hint("move-right", "completions"),
                  hint("accept", "use completion"),
                  hint("back", "back"),
                  hint("cancel", "close"),
                ]
              : helpContext === "error-overlay"
                ? [hint("dismiss-overlay", "dismiss"), hint("back", "back")]
                : [];

  const footer = <div className="help">{help}</div>;
  // Like the terminal UI: the name of the previewed project, then the tab filter.
  const previewRoot = previewData?.kind === "local" ? previewData.rootPath : entry?.metadataPath;
  const previewTitle = previewRoot
    ? `${previewRoot.split("/").pop() ?? previewRoot}${worktreeFilter ? ` / ${worktreeFilter}` : ""}`
    : "Preview";

  return (
    <div className="app">
      <main className="layout" data-detail={detailTabs.length > 0}>
        <ResultsPanel
          rows={rows}
          selectedIndex={selectedIndex}
          query={query}
          title={resultsTitle}
          status={status}
          focused={effectiveFocus === "search" && !overlay}
          inputRef={searchRef}
          onQuery={(value) => {
            setQuery(value);
            setStickToFirst(true);
          }}
          onFocus={() => setFocus("search")}
          onSelect={selectIndex}
          onTagClick={filterByTag}
          onActivate={(index) => {
            selectIndex(index);
            startTarget({ kind: "actions" }).catch(showError);
          }}
        />
        <div className="side">
          <PreviewPanel
            title={previewTitle}
            preview={preview}
            tabs={visibleTabs.map((index) => tabs[index]).filter((tab) => tab !== undefined)}
            activeTabIndex={visibleTabs.indexOf(activeTabIndex)}
            tree={tree}
            tags={entry ? entry.tags.map((tag) => tag.name) : null}
            query={query}
            tagEdit={effectiveFocus === "tag-edit" ? tagEdit : null}
            tagInputRef={tagInputRef}
            focused={(effectiveFocus === "preview" || effectiveFocus === "tag-edit") && !overlay}
            scrollRef={previewScrollRef}
            onFocus={() => {
              if (effectiveFocus !== "tag-edit") setFocus("preview");
            }}
            onSelectTab={(index) => {
              setTabVisibleIndex(index);
              setDetailIndex(0);
            }}
            onTagClick={filterByTag}
            onEditTags={() => editTags().catch(showError)}
            onTagEdit={setTagEdit}
            onSaveTags={() => tagEdit && confirmTags(tagEdit).catch(showError)}
            onCancelTags={cancelTags}
          />
          <DetailPanel
            tabs={detailTabs}
            activeIndex={Math.min(detailIndex, Math.max(detailTabs.length - 1, 0))}
            focused={effectiveFocus === "detail" && !overlay}
            scrollRef={detailScrollRef}
            onFocus={() => setFocus("detail")}
            onSelectTab={setDetailIndex}
          />
        </div>
      </main>
      <footer className="footer">
        {!overlay && footer}
        {notice && (
          <span className="notice" data-error={notice.error}>
            {notice.text}
          </span>
        )}
      </footer>

      {overlay?.kind === "actions" && (
        <ActionPicker
          path={overlay.path}
          actions={overlay.visible.map((index) => bootstrap.actions[index]).filter((action) => action !== undefined)}
          selected={overlay.selected}
          query={overlay.query}
          footer={footer}
          onPick={(index) => {
            const view = bootstrap.actions[overlay.visible[index] ?? -1];
            if (view) runAction({ by: "index", index: view.index }, overlay.path, view.label).catch(showError);
          }}
        />
      )}
      {overlay?.kind === "create-picker" && (
        <CreatePicker
          items={filterCreates(bootstrap, overlay.query)}
          selected={overlay.selected}
          query={overlay.query}
          footer={footer}
          onPick={(index) => {
            const item = filterCreates(bootstrap, overlay.query)[index];
            if (item) openCreateForm(item.index, overlay.path).catch(showError);
          }}
        />
      )}
      {overlay?.kind === "create-form" && (
        <CreateForm
          item={createItem(overlay.form)}
          form={overlay.form}
          footer={footer}
          onInput={(value) => {
            const form = overlay.form;
            const values = form.values.map((current, index) => (index === form.active ? value : current));
            updateForm({ ...form, values, error: null }, true).catch(showError);
          }}
          onActivate={(active) => updateForm({ ...overlay.form, active, completions: false }, false).catch(showError)}
          onPickSuggestion={(index) => {
            const form = overlay.form;
            const suggestion = form.suggestions[index];
            if (!suggestion) return;
            const values = form.values.map((value, valueIndex) => (valueIndex === form.active ? suggestion.value : value));
            updateForm({ ...form, values, completions: false }, false).catch(showError);
          }}
        />
      )}
      {overlay?.kind === "progress" && (
        <ProgressOverlay title={overlay.title} messages={overlay.messages} error={overlay.error} footer={footer} />
      )}
    </div>
  );
}
