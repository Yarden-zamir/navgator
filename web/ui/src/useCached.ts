import { useEffect, useState } from "react";

export type Loaded<T> =
  | { state: "loading" }
  | { state: "ready"; value: T }
  | { state: "failed"; error: string };

/** A per-key cache for data that does not change while the page is open. */
export class Cache<T> {
  private readonly entries = new Map<string, Promise<T>>();

  constructor(private readonly load: (key: string) => Promise<T>) {}

  get(key: string): Promise<T> {
    let entry = this.entries.get(key);
    if (!entry) {
      entry = this.load(key);
      // A failed load must not stick; the next request retries.
      entry.catch(() => this.entries.delete(key));
      this.entries.set(key, entry);
    }
    return entry;
  }

  clear(): void {
    this.entries.clear();
  }
}

/** Load `key` through `cache`; `null` means there is nothing to load. */
export function useCached<T>(cache: Cache<T>, key: string | null): Loaded<T> | null {
  const [loaded, setLoaded] = useState<{ key: string; value: Loaded<T> } | null>(null);
  useEffect(() => {
    if (key === null) return;
    let current = true;
    setLoaded({ key, value: { state: "loading" } });
    cache.get(key).then(
      (value) => current && setLoaded({ key, value: { state: "ready", value } }),
      (error: unknown) =>
        current && setLoaded({ key, value: { state: "failed", error: String(error) } }),
    );
    return () => {
      current = false;
    };
  }, [cache, key]);
  if (key === null) return null;
  return loaded?.key === key ? loaded.value : { state: "loading" };
}
