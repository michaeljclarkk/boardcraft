// IndexedDB is exact-origin scoped. Unlike Web Storage its changes do not
// distribute the old/new credential halves to listening windows.
const opened = new Promise((ok, fail) => {
  const req = indexedDB.open('boardcraft-preview', 1);
  req.onupgradeneeded = () => req.result.createObjectStore('session');
  req.onsuccess = () => ok(req.result);
  req.onerror = () => fail(req.error);
});
const changes = new BroadcastChannel('boardcraft-session-changed');
export const onChange = callback => { changes.onmessage = callback; };

export async function readBinding() {
  const db = await opened;
  return new Promise((ok, fail) => {
    const req = db.transaction('session').objectStore('session').get('binding');
    req.onsuccess = () => ok(req.result);
    req.onerror = () => fail(req.error);
  });
}

// A delayed rejection must not forget a replacement login.
export async function changeBinding(expected, next) {
  const db = await opened;
  return new Promise((ok, fail) => {
    const tx = db.transaction('session','readwrite'), store = tx.objectStore('session');
    const req = store.get('binding');
    let changed = false;
    req.onsuccess = () => {
      if (req.result !== expected) return;
      if (next) store.put(next,'binding'); else store.delete('binding');
      changed = true;
    };
    tx.oncomplete = () => { if (changed) changes.postMessage(null); ok(changed); };
    tx.onabort = () => fail(tx.error);
  });
}
