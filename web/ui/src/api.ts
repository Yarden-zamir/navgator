// Typed calls to the `navgator web` server. The types in ./api/ are generated
// from the Rust structs (`cargo test`); never edit them by hand.

import type { ActionRef } from "./api/ActionRef";
import type { Bootstrap } from "./api/Bootstrap";
import type { ErrorBody } from "./api/ErrorBody";
import type { GitSummary } from "./api/GitSummary";
import type { GithubView } from "./api/GithubView";
import type { MissingCreateValue } from "./api/MissingCreateValue";
import type { PathSuggestion } from "./api/PathSuggestion";
import type { Preview } from "./api/Preview";
import type { Progress } from "./api/Progress";
import type { Results } from "./api/Results";
import type { RunResult } from "./api/RunResult";
import type { SettingsUpdate } from "./api/SettingsUpdate";
import type { SettingsView } from "./api/SettingsView";
import type { SortMode } from "./api/SortMode";
import type { Target } from "./api/Target";
import type { TreeView } from "./api/TreeView";

export class ApiError extends Error {}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, {
    method,
    headers: body === undefined ? {} : { "content-type": "application/json" },
    body: body === undefined ? null : JSON.stringify(body),
  });
  if (!response.ok) throw new ApiError(await errorMessage(response));
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

async function errorMessage(response: Response): Promise<string> {
  const text = await response.text();
  try {
    return (JSON.parse(text) as ErrorBody).error;
  } catch {
    return text || `${response.status} ${response.statusText}`;
  }
}

function query(params: Record<string, string | boolean | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) search.set(key, String(value));
  }
  return search.toString();
}

export const api = {
  settings: () => request<SettingsView>("GET", "/api/settings"),
  saveSettings: (update: SettingsUpdate) => request<SettingsView>("PUT", "/api/settings", update),
  bootstrap: () => request<Bootstrap>("GET", "/api/bootstrap"),
  results: (q: string, sort: SortMode, remotes: boolean) =>
    request<Results>("GET", `/api/results?${query({ q, sort, remotes })}`),
  loadRemotes: (entryId: string) => request<void>("POST", "/api/remotes", { entryId }),
  preview: (entry: string) => request<Preview>("GET", `/api/preview?${query({ entry })}`),
  tree: (path: string) => request<TreeView>("GET", `/api/tree?${query({ path })}`),
  git: (path: string, selectedRepoIsBare: boolean) =>
    request<GitSummary | null>(
      "GET",
      `/api/git?${query({ path, selectedRepoIsBare })}`,
    ),
  github: (path: string) => request<GithubView | null>("GET", `/api/github?${query({ path })}`),
  target: (entry: string, tab: string | undefined) =>
    request<Target>("GET", `/api/target?${query({ entry, tab })}`),
  actions: (path: string, q: string) =>
    request<number[]>("GET", `/api/actions?${query({ path, q })}`),
  runAction: (action: ActionRef, path: string) =>
    request<RunResult>("POST", "/api/actions/run", { action, path }),
  createValues: (index: number, path: string | null, values: string[] | null, activePrompt = 0) =>
    request<string[]>("POST", "/api/create/values", { index, path, values, activePrompt }),
  pathCompletions: (input: string) =>
    request<PathSuggestion[]>("GET", `/api/paths?${query({ input })}`),
  saveTags: (path: string, tags: string[]) => request<void>("PUT", "/api/tags", { path, tags }),
  tagSuggestions: () => request<string[]>("GET", "/api/tags/suggestions"),
};

/** Long operations stream `Progress` lines; this resolves with the final path. */
export async function streamProgress(
  path: string,
  body: unknown,
  onMessage: (text: string) => void,
): Promise<string> {
  const response = await postJson(path, body);
  if (!response.ok) throw new ApiError(await errorMessage(response));
  return readProgress(response, onMessage);
}

/** Create recipes answer 422 with the prompt that needs a value. */
export async function runCreate(
  body: { index: number; path: string | null; values: string[] },
  onMessage: (text: string) => void,
): Promise<{ path: string } | { missing: MissingCreateValue }> {
  const response = await postJson("/api/create/run", body);
  if (response.status === 422) return { missing: (await response.json()) as MissingCreateValue };
  if (!response.ok) throw new ApiError(await errorMessage(response));
  return { path: await readProgress(response, onMessage) };
}

function postJson(path: string, body: unknown): Promise<Response> {
  return fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function readProgress(response: Response, onMessage: (text: string) => void): Promise<string> {
  if (!response.body) throw new ApiError("The server sent no progress stream");
  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += value;
    let newline = buffer.indexOf("\n");
    while (newline >= 0) {
      const progress = JSON.parse(buffer.slice(0, newline)) as Progress;
      buffer = buffer.slice(newline + 1);
      switch (progress.type) {
        case "message":
          onMessage(progress.text);
          break;
        case "done":
          return progress.path;
        case "error":
          throw new ApiError(progress.message);
      }
      newline = buffer.indexOf("\n");
    }
  }
  throw new ApiError("The server stopped before the operation finished");
}
