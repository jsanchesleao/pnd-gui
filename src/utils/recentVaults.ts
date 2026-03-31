export interface RecentVaultEntry {
  id: number;
  name: string;
  handle: FileSystemDirectoryHandle;
  lastOpened: number;
  favorite: boolean;
}

const DB_NAME = "pnd-recent-vaults";
const DB_VERSION = 1;
const STORE_NAME = "vaults";
const NON_FAVORITE_LIMIT = 5;

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

export async function getRecentVaults(): Promise<RecentVaultEntry[]> {
  const db = await openDb();
  const tx = db.transaction(STORE_NAME, "readonly");
  const all: RecentVaultEntry[] = await idbRequest(tx.objectStore(STORE_NAME).getAll());
  await txComplete(tx);
  db.close();

  const favorites = all
    .filter((e) => e.favorite)
    .sort((a, b) => b.lastOpened - a.lastOpened);
  const nonFavorites = all
    .filter((e) => !e.favorite)
    .sort((a, b) => b.lastOpened - a.lastOpened);

  return [...favorites, ...nonFavorites];
}

export async function addRecentVault(
  name: string,
  handle: FileSystemDirectoryHandle,
): Promise<void> {
  // Step 1: read all in a readonly transaction, then close it before any non-IDB async
  const db = await openDb();
  const readTx = db.transaction(STORE_NAME, "readonly");
  const all: RecentVaultEntry[] = await idbRequest(readTx.objectStore(STORE_NAME).getAll());
  await txComplete(readTx);

  // Step 2: isSameEntry checks — outside any transaction
  let existing: RecentVaultEntry | undefined;
  for (const entry of all) {
    if (await handle.isSameEntry(entry.handle)) {
      existing = entry;
      break;
    }
  }

  const now = Date.now();

  // Step 3: apply writes in a fresh readwrite transaction
  const writeTx = db.transaction(STORE_NAME, "readwrite");
  const store = writeTx.objectStore(STORE_NAME);

  if (existing) {
    await idbRequest(store.put({ ...existing, name, handle, lastOpened: now }));
  } else {
    await idbRequest(store.add({ name, handle, lastOpened: now, favorite: false }));

    // Enforce non-favorite limit within the same transaction
    const allAfterAdd: RecentVaultEntry[] = await idbRequest(store.getAll());
    const nonFavs = allAfterAdd
      .filter((e) => !e.favorite)
      .sort((a, b) => a.lastOpened - b.lastOpened); // ascending: oldest first

    const toDelete = nonFavs.slice(0, nonFavs.length - NON_FAVORITE_LIMIT);
    for (const entry of toDelete) {
      await idbRequest(store.delete(entry.id));
    }
  }

  await txComplete(writeTx);
  db.close();
}

export async function removeRecentVault(id: number): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_NAME, "readwrite");
  await idbRequest(tx.objectStore(STORE_NAME).delete(id));
  await txComplete(tx);
  db.close();
}

export async function toggleFavorite(id: number): Promise<void> {
  // Step 1: read all (readonly tx), then close
  const db = await openDb();
  const readTx = db.transaction(STORE_NAME, "readonly");
  const all: RecentVaultEntry[] = await idbRequest(readTx.objectStore(STORE_NAME).getAll());
  await txComplete(readTx);

  const target = all.find((e) => e.id === id);
  if (!target) { db.close(); return; }

  const newFavorite = !target.favorite;

  // Step 2: apply toggle and optional overflow cleanup
  const writeTx = db.transaction(STORE_NAME, "readwrite");
  const store = writeTx.objectStore(STORE_NAME);

  await idbRequest(store.put({ ...target, favorite: newFavorite }));

  if (!newFavorite) {
    // Un-favoriting: target re-enters the non-favorite pool.
    // nonFavs = others that were already non-favorites (sorted ascending = oldest first)
    const nonFavs = all
      .filter((e) => !e.favorite && e.id !== id)
      .sort((a, b) => a.lastOpened - b.lastOpened);

    // After toggle: total non-favs = nonFavs.length + 1.
    // Delete oldest until total is at limit.
    const toDelete = nonFavs.slice(0, nonFavs.length - NON_FAVORITE_LIMIT + 1);
    for (const entry of toDelete) {
      await idbRequest(store.delete(entry.id));
    }
  }

  await txComplete(writeTx);
  db.close();
}
