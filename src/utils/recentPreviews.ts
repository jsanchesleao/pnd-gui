export interface RecentPreviewEntry {
  id: number;
  type: "local" | "remote";
  handle?: FileSystemFileHandle; // local only — stored natively in IDB
  url?: string;                  // remote only
  alias?: string;
  lastOpened: number;
}

const DB_NAME = "pnd-recent-previews";
const DB_VERSION = 1;
const STORE_NAME = "previews";
const LIMIT = 10;

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: "id", autoIncrement: true });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function idbRequest<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function txComplete(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

export async function getRecentPreviews(): Promise<RecentPreviewEntry[]> {
  const db = await openDb();
  const tx = db.transaction(STORE_NAME, "readonly");
  const all: RecentPreviewEntry[] = await idbRequest(tx.objectStore(STORE_NAME).getAll());
  await txComplete(tx);
  db.close();
  return all.sort((a, b) => b.lastOpened - a.lastOpened);
}

async function enforceLimit(store: IDBObjectStore): Promise<void> {
  const all: RecentPreviewEntry[] = await idbRequest(store.getAll());
  const sorted = all.sort((a, b) => a.lastOpened - b.lastOpened); // oldest first
  const toDelete = sorted.slice(0, sorted.length - LIMIT);
  for (const entry of toDelete) {
    await idbRequest(store.delete(entry.id));
  }
}

export async function addLocalRecentPreview(
  handle: FileSystemFileHandle,
): Promise<void> {
  const db = await openDb();
  const readTx = db.transaction(STORE_NAME, "readonly");
  const all: RecentPreviewEntry[] = await idbRequest(readTx.objectStore(STORE_NAME).getAll());
  await txComplete(readTx);

  let existing: RecentPreviewEntry | undefined;
  for (const entry of all) {
    if (entry.type === "local" && entry.handle && await handle.isSameEntry(entry.handle)) {
      existing = entry;
      break;
    }
  }

  const now = Date.now();
  const writeTx = db.transaction(STORE_NAME, "readwrite");
  const store = writeTx.objectStore(STORE_NAME);

  if (existing) {
    await idbRequest(store.put({ ...existing, handle, lastOpened: now }));
  } else {
    await idbRequest(store.add({ type: "local", handle, lastOpened: now }));
    await enforceLimit(store);
  }

  await txComplete(writeTx);
  db.close();
}

export async function addRemoteRecentPreview(url: string): Promise<void> {
  const db = await openDb();
  const readTx = db.transaction(STORE_NAME, "readonly");
  const all: RecentPreviewEntry[] = await idbRequest(readTx.objectStore(STORE_NAME).getAll());
  await txComplete(readTx);

  const existing = all.find((e) => e.type === "remote" && e.url === url);
  const now = Date.now();

  const writeTx = db.transaction(STORE_NAME, "readwrite");
  const store = writeTx.objectStore(STORE_NAME);

  if (existing) {
    await idbRequest(store.put({ ...existing, lastOpened: now }));
  } else {
    await idbRequest(store.add({ type: "remote", url, lastOpened: now }));
    await enforceLimit(store);
  }

  await txComplete(writeTx);
  db.close();
}

export async function removeRecentPreview(id: number): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_NAME, "readwrite");
  await idbRequest(tx.objectStore(STORE_NAME).delete(id));
  await txComplete(tx);
  db.close();
}

export async function renameRecentPreview(id: number, alias: string): Promise<void> {
  const db = await openDb();
  const readTx = db.transaction(STORE_NAME, "readonly");
  const all: RecentPreviewEntry[] = await idbRequest(readTx.objectStore(STORE_NAME).getAll());
  await txComplete(readTx);

  const target = all.find((e) => e.id === id);
  if (!target) { db.close(); return; }

  const writeTx = db.transaction(STORE_NAME, "readwrite");
  await idbRequest(
    writeTx.objectStore(STORE_NAME).put({ ...target, alias: alias.trim() || undefined }),
  );
  await txComplete(writeTx);
  db.close();
}
