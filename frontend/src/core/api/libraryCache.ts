/** Display snapshots only. Never store credentials or full library connection settings. */
export class LibraryViewCache {
  private entries = new Map<string, string>();
  private bytes = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private readonly storage?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
  private readonly storageKey = 'ting-library-views-v1';
  private readonly maxBytes: number;

  constructor(storage?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>, maxBytes = 4 * 1024 * 1024) {
    this.storage = storage;
    this.maxBytes = maxBytes;
    try {
      const raw = storage?.getItem(this.storageKey);
      if (!raw || raw.length * 2 > maxBytes) return;
      const saved: unknown = JSON.parse(raw);
      if (!Array.isArray(saved)) return;
      for (const entry of saved) {
        if (Array.isArray(entry) && entry.length === 2 && typeof entry[0] === 'string' && typeof entry[1] === 'string') {
          this.remember(entry[0], entry[1]);
        }
      }
    } catch { /* Unavailable or corrupt storage must not block the library. */ }
  }

  private key(identity: string, resource: string) { return JSON.stringify([identity, resource]); }

  get<T>(identity: string, resource: string): T | null {
    if (!identity) return null;
    const key = this.key(identity, resource);
    const raw = this.entries.get(key);
    if (!raw) return null;
    try {
      this.entries.delete(key);
      this.entries.set(key, raw);
      return JSON.parse(raw) as T;
    } catch { return null; }
  }

  set(identity: string, resource: string, value: unknown) {
    if (!identity) return;
    try {
      this.remember(this.key(identity, resource), JSON.stringify(value));
      clearTimeout(this.timer);
      this.timer = setTimeout(() => this.flush(), 250);
    } catch { /* Cache failures never prevent viewing or editing books. */ }
  }

  private remember(key: string, raw: string) {
    this.bytes -= (this.entries.get(key)?.length ?? 0) * 2 + (this.entries.has(key) ? key.length * 2 : 0);
    this.entries.delete(key);
    const size = (key.length + raw.length) * 2;
    if (size > Math.min(this.maxBytes, 1024 * 1024)) return;
    this.entries.set(key, raw);
    this.bytes += size;
    while (this.entries.size > 16 || this.bytes > this.maxBytes) {
      const oldest = this.entries.entries().next().value!;
      this.bytes -= (oldest[0].length + oldest[1].length) * 2;
      this.entries.delete(oldest[0]);
    }
  }

  flush() {
    clearTimeout(this.timer);
    this.timer = undefined;
    try { this.storage?.setItem(this.storageKey, JSON.stringify([...this.entries])); }
    catch {
      try { this.storage?.removeItem(this.storageKey); } catch { /* Storage can be disabled. */ }
    }
  }

  dispose() { this.flush(); this.entries.clear(); this.bytes = 0; }
}

export const libraryCacheIdentity = (server: string, userId?: string) =>
  userId ? JSON.stringify([server.replace(/\/+$/, ''), userId]) : '';

const sessionStorageOrUndefined = () => {
  try { return typeof window === 'undefined' ? undefined : window.sessionStorage; }
  catch { return undefined; }
};
export const libraryViewCache = new LibraryViewCache(sessionStorageOrUndefined());
