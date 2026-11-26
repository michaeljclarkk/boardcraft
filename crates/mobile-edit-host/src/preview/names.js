// State belongs to one mounted board visit. Requests close over that visit's path;
// only its accepted rows supply editor baselines. A projection never changes them.
export function mountNames(board, list, failures, api) {
  const rejected = new Map();
  let busy = false;

  function button(label, click) {
    const b = document.createElement('button');
    b.textContent = label; b.onclick = click;
    return b;
  }

  function showFailures() {
    failures.replaceChildren();
    for (const [id, edit] of rejected) {
      const row = board.rows.find(r => r.id === id);
      if (!row) continue;
      const message = document.createElement('p');
      message.setAttribute('role', 'alert');
      message.textContent = `Could not rename ${row.title}.`;
      failures.append(message, button('Retry change', () => retry(id, edit)), button('Dismiss', () => {
        if (!busy) { rejected.delete(id); showFailures(); }
      }));
    }
  }

  function failed(error, fallback) {
    if (error === 401) api.expired();
    else if (error === 403) api.denied();
    else api.reload(fallback);
  }

  async function retry(id, edit) {
    if (busy || !api.current()) return;
    busy = true;
    try {
      board = await api.read();
      if (!api.current()) return;
      if (!board.editable || !board.rows.some(r => r.id === id)) {
        rejected.clear(); render(); showFailures();
        document.querySelector('main > [role=alert]').textContent = 'Name change unavailable.';
        return;
      }
      render();
      await save(id, edit);
    } catch (error) {
      if (api.current()) failed(error, 'Could not reload board.');
    } finally { busy = false; }
  }

  async function save(id, edit) {
    busy = true;
    rejected.delete(id); showFailures();
    const accepted = board.rows.find(r => r.id === id);
    const display = list.querySelector(`[data-item-id="${CSS.escape(id)}"] button`);
    display.textContent = edit.title;
    try {
      const saved = await api.save(id, edit);
      if (!api.current()) return;
      if (saved.id !== id || typeof saved.title !== 'string' || !Number.isSafeInteger(saved.revision) || saved.revision < 1) {
        throw Error('Unexpected saved item');
      }
      Object.assign(accepted, saved);
      display.textContent = accepted.title;
    } catch (error) {
      if (!api.current()) return;
      if (error === 422) {
        display.textContent = accepted.title;
        rejected.set(id, edit); showFailures();
      } else failed(error, error === 409 ? 'Name changed. Reload the board.' : 'Save outcome unavailable. Reload the board.');
    } finally { busy = false; }
  }

  function cell(row) {
    const li = document.createElement('li');
    li.dataset.itemId = row.id;
    if (!board.editable) { li.textContent = row.title; return li; }
    const display = button(row.title, e => { if (e.detail === 0) open(); });
    display.ondblclick = open;
    display.onkeydown = e => { if (e.key === 'F2') { e.preventDefault(); open(); } };
    function open() {
      if (busy || !api.current()) return;
      const {title, revision} = row;
      const input = document.createElement('input');
      input.setAttribute('aria-label', 'Edit item name'); input.value = title;
      function close(submit) {
        input.onblur = null;
        if (!api.current()) return;
        input.replaceWith(display);
        if (submit && input.value !== title) save(row.id, {title: input.value, revision});
      }
      input.onblur = () => close(true);
      input.onkeydown = e => {
        if (e.key === 'Escape') { e.preventDefault(); close(false); }
        if (e.key === 'Enter' && !e.isComposing) { e.preventDefault(); input.blur(); }
      };
      display.replaceWith(input); input.focus(); input.select();
    }
    li.append(display); return li;
  }
  function render() { list.replaceChildren(...board.rows.map(cell)); }
  render();
}
