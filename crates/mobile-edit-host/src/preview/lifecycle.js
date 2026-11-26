function element(tag, text) {
  const node = document.createElement(tag);
  node.textContent = text;
  return node;
}

export function archiveDialog(row, {api, board, busy, setBusy, refresh, button}, container) {
  if (!api.current() || busy() || !board().owner) return;
  let draft = {operation_id: crypto.randomUUID(), board_revision: board().revision, item_revision: row.revision};
  const dialog = document.createElement('dialog');
  const title = 'Archive selected items?';
  dialog.setAttribute('aria-label', title);
  const description = element('p', `1 item: ${row.title}. Restore it later from this board’s Archive.`);
  const status = element('p', ''); status.setAttribute('role', 'status');
  const confirm = button('Confirm', submit), cancel = button('Cancel', () => dialog.close());
  const review = button('Review latest items', async () => {
    if (busy() || !api.current()) return;
    setBusy(true); review.disabled = true;
    try {
      const latest = await api.read();
      if (!api.current()) return;
      refresh(latest);
      const current = latest.rows.find(r => r.id === row.id);
      if (!current || !latest.owner) { status.textContent = current ? 'Owner access required.' : 'Item unavailable.'; return; }
      draft = {...draft, board_revision: latest.revision, item_revision: current.revision};
      description.textContent = `1 item: ${current.title}. Restore it later from this board’s Archive.`;
      status.textContent = 'Latest revisions loaded. Confirm to archive.';
      review.hidden = true; confirm.disabled = false;
    } catch (error) { if (api.current()) failure(error, 'Could not review item.'); }
    finally { if (api.current()) { setBusy(false); review.disabled = false; } }
  });
  function failure(error, message) {
    if (error === 401) api.expired();
    else if (error === 403) api.denied('Archive denied.');
    else status.textContent = message;
  }
  async function submit() {
    if (busy() || !api.current()) return;
    setBusy(true); confirm.disabled = cancel.disabled = true; status.textContent = 'Saving…';
    try {
      await api.transition(row.id, 'archive', draft);
    } catch (error) {
      if (api.current()) {
        if (error === 500) { failure(error, 'Could not archive item.'); confirm.disabled = false; }
        else { failure(error, error === 409 ? 'Item changed. Review latest items.' : 'Archive outcome unavailable. Review latest items.'); review.hidden = false; }
        setBusy(false); cancel.disabled = false;
      }
      return;
    }
    if (!api.current()) return;
    dialog.remove();
    try { const latest = await api.read(); if (api.current()) refresh(latest); }
    catch (error) { if (api.current()) { if (error === 401 || error === 403) failure(error, ''); else api.reload('Item archived. Reload the board.'); } }
    finally { if (api.current()) setBusy(false); }
  }
  review.hidden = true;
  dialog.oncancel = event => { if (busy()) event.preventDefault(); };
  dialog.onclose = () => dialog.remove();
  dialog.append(element('h2', title), description, confirm, cancel, review, status);
  container.append(dialog); dialog.showModal(); cancel.focus();
}

export function mountArchive(container, page, api) {
  const count = page.total;
  container.append(element('h2', 'Items'), element('p', count ? `${count} archived item${count === 1 ? '' : 's'}` : 'No archived items'));
  if (!page.can_manage) container.append(element('p', 'Owner access required.'));
  if (page.has_more) container.append(element('p', `Showing the first ${page.items.length} items.`));
  const list = document.createElement('ul'); container.append(list);
  let busy = false;
  for (const row of page.items) {
    const item = document.createElement('li'); item.dataset.itemId = row.id;
    const restore = element('button', 'Restore item'); restore.disabled = !row.restorable;
    item.append(element('span', `${row.title} · ${row.group_name}`), restore);
    if (!row.group_active) item.append(element('span', 'Restore its group first.'));
    list.append(item);
    restore.onclick = async () => {
      if (busy || !api.current()) return;
      busy = true; restore.textContent = 'Restoring…';
      for (const b of list.querySelectorAll('button')) b.disabled = true;
      try {
        await api.transition(row.id, 'restore', {operation_id: crypto.randomUUID(), board_revision: page.board_revision, item_revision: row.revision});
        if (!api.current()) return;
        await api.readBoard();
        if (api.current()) api.refresh();
      } catch (error) {
        if (!api.current()) return;
        if (error === 401) return api.expired();
        if (error === 403) return api.refresh();
        restore.textContent = 'Restore item';
        container.append(element('p', error === 409 ? 'Archive changed.' : 'Restore outcome unavailable.'));
        const reload = element('button', 'Reload recovery list'); reload.onclick = api.refresh;
        container.append(reload); reload.focus();
      }
    };
  }
}
