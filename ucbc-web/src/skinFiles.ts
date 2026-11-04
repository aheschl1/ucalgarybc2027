import { unzipSync, zipSync } from "fflate";

/** Replacement pictures as data URLs, keyed by file under the assets folder. */
export type Overrides = Record<string, string>;

const SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const PREFIX = "data:image/png;base64,";

export function isPng(bytes: Uint8Array): boolean {
  return SIGNATURE.every((b, i) => bytes[i] === b);
}

export function toDataUrl(bytes: Uint8Array): string {
  let text = "";
  for (const b of bytes) text += String.fromCharCode(b);
  return PREFIX + btoa(text);
}

function fromDataUrl(href: string): Uint8Array {
  return Uint8Array.from(atob(href.slice(PREFIX.length)), (c) => c.charCodeAt(0));
}

export function toZip(overrides: Overrides): Uint8Array {
  const files = Object.entries(overrides).map(([file, href]) => [file, fromDataUrl(href)]);
  return zipSync(Object.fromEntries(files), { level: 0 });
}

/** The PNGs at the paths in `known`, and the name of everything else. Throws if `zip` isn't one. */
export function fromZip(zip: Uint8Array, known: Set<string>): { found: Overrides; skipped: string[] } {
  const found: Overrides = {};
  const skipped: string[] = [];
  for (const [name, bytes] of Object.entries(unzipSync(zip))) {
    if (name.endsWith("/")) continue;
    if (known.has(name) && isPng(bytes)) found[name] = toDataUrl(bytes);
    else skipped.push(name);
  }
  return { found, skipped };
}

const STORE = "pictures";

function request<T>(r: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });
}

async function database(): Promise<IDBDatabase> {
  const opening = indexedDB.open("ucbc-skin", 1);
  opening.onupgradeneeded = () => opening.result.createObjectStore(STORE);
  return request(opening);
}

export async function loadOverrides(): Promise<Overrides> {
  const db = await database();
  try {
    const store = db.transaction(STORE).objectStore(STORE);
    const [keys, values] = await Promise.all([request(store.getAllKeys()), request(store.getAll())]);
    return Object.fromEntries(keys.map((key, i) => [String(key), values[i] as string]));
  } finally {
    db.close();
  }
}

/** Stores each picture in `changes`, and forgets the ones that are `null`. */
export async function saveOverrides(changes: Record<string, string | null>): Promise<void> {
  const db = await database();
  try {
    const tx = db.transaction(STORE, "readwrite");
    const store = tx.objectStore(STORE);
    for (const [file, href] of Object.entries(changes)) {
      if (href === null) store.delete(file);
      else store.put(href, file);
    }
    await new Promise<void>((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = tx.onabort = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}
