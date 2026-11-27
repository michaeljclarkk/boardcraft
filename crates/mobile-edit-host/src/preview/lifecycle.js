function element(tag, text) {
  const node = document.createElement(tag);
  node.textContent = text;
  return node;
}

export function archiveDialog(targets, {api, board, busy, setBusy, refresh, button}, container, clearSelection, deleting = false) {
  if (!api.current() || busy() || !board().owner) return;
  const batch = Array.isArray(targets), rows = batch ? targets : [targets];
  if (!rows.length || rows.some(r => !r)) return;
  const ids = rows.map(r => r.id), row = rows[0];
  let draft = {operation_id: crypto.randomUUID(), board_revision: board().revision,
    ...(batch ? {items: rows.map(r => ({id:r.id, revision:r.revision}))} : {item_revision:row.revision})};
  const dialog = document.createElement('dialog');
  const verb = deleting ? 'Delete' : 'Archive', destination = deleting ? 'Trash' : 'Archive';
  const title = `${verb} selected items?`;
  dialog.setAttribute('aria-label', title);
  const summary = rows => `${rows.length} item${rows.length === 1 ? '' : 's'}: ${rows.slice(0,6).map(r=>r.title).join(', ')}${rows.length>6 ? ` and ${rows.length-6} more` : ''}. Restore ${rows.length===1?'it':'them'} later from this board’s ${destination}.`;
  const description = element('p', summary(rows));
  let reviewed;
  const detail = element('div', '');
  const adopt = button('Use these revisions and keep my choices', () => {
    if (!reviewed || busy() || !api.current()) return;
    draft = {...draft, board_revision:reviewed.revision, items:reviewed.rows.map(r=>({id:r.id,revision:r.revision}))};
    description.textContent = summary(reviewed.rows); reviewed = undefined;
    detail.replaceChildren(); adopt.hidden = review.hidden = true; confirm.disabled = false;
    status.textContent = `Latest revisions loaded. Confirm to ${verb.toLowerCase()}.`;
  });
  adopt.hidden = true;
  const status = element('p', ''); status.setAttribute('role', 'status');
  const trigger = document.activeElement;
  const confirm = button(deleting ? 'Delete' : 'Confirm', submit), cancel = button('Cancel', () => { if (!busy()) dialog.close(); });
  const review = button('Review latest items', async () => {
    if (busy() || !api.current()) return;
    setBusy(true); review.disabled = true; reviewed = undefined; adopt.disabled = true;
    try {
      const latest = await api.read();
      if (!api.current()) return;
      if (batch) {
        if (!latest.owner) { dialog.remove(); refresh(latest); return; }
        const current = ids.map(id => latest.rows.find(r=>r.id===id));
        reviewed = current.every(Boolean) && {revision:latest.revision,rows:current};
        detail.replaceChildren(...current.map((r,i)=>element('p', r ? `${r.title} (${ids[i]}), revision ${r.revision}` : `${ids[i]}: Unavailable`)));
        adopt.hidden = false; adopt.disabled = !reviewed;
        status.textContent = 'Review every requested item before adopting these revisions.';
        return;
      }
      refresh(latest);
      const current = latest.rows.find(r => r.id === row.id);
      if (!current || !latest.owner) { status.textContent = current ? 'Owner access required.' : 'Item unavailable.'; return; }
      draft = {...draft, board_revision: latest.revision, item_revision: current.revision};
      description.textContent = `1 item: ${current.title}. Restore it later from this board’s Archive.`;
      status.textContent = 'Latest revisions loaded. Confirm to archive.';
      review.hidden = true; confirm.disabled = false;
    } catch (error) { if (api.current()) failure(error, batch ? 'Could not review selected items.' : 'Could not review item.'); }
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
      if (batch) {
        const ack = await (deleting ? api.deleteSelection(draft) : api.archiveSelection(draft));
        if (ack.operation_id!==draft.operation_id || ack.board_id!==board().id || !Array.isArray(ack.affected_ids) || ack.affected_ids.length!==ids.length || new Set(ack.affected_ids).size!==ids.length || !ack.affected_ids.every(id=>ids.includes(id))) throw Error('Unexpected acknowledgment');
      } else await api.transition(row.id, 'archive', draft);
    } catch (error) {
      if (api.current()) {
        if (batch) {
          const uncertain = ![401,403,404,409,422,500].includes(error);
          if (error===403) { dialog.remove(); try { const latest=await api.read(); if(api.current()) refresh(latest); } catch(e) { if(api.current()) { if(e===401) api.expired(); else api.reload(e===403 || e===404 ? 'Destination unavailable' : 'Could not recheck access. Reload the board.'); } } }
           else if (uncertain) api.reload(`${verb} outcome unavailable. Reload the board.`);
           else { failure(error, error===409 ? 'Items changed. Review latest items.' : `${verb} not confirmed. Review latest items.`); review.hidden=false; }
        }
        else if (error === 500) { failure(error, 'Could not archive item.'); confirm.disabled = false; }
        else { failure(error, error === 409 ? 'Item changed. Review latest items.' : 'Archive outcome unavailable. Review latest items.'); review.hidden = false; }
        setBusy(false); cancel.disabled = false;
      }
      return;
    }
    if (!api.current()) return;
    dialog.remove();
    try { const latest = await api.read(); if (api.current()) { clearSelection?.(); refresh(latest); } }
    catch (error) { if (api.current()) { if (error === 401 || error === 403) failure(error, ''); else api.reload(batch ? `Selected items ${deleting?'deleted':'archived'}. Reload the board.` : 'Item archived. Reload the board.'); } }
    finally { if (api.current()) setBusy(false); }
  }
  review.hidden = true;
  dialog.oncancel = event => { if (busy()) event.preventDefault(); };
  dialog.onclose = () => { dialog.remove(); if (trigger?.isConnected) trigger.focus(); };
  dialog.append(element('h2', title), description, confirm, cancel, review, status, detail, adopt);
  container.append(dialog); dialog.showModal(); cancel.focus();
}

export function mountArchive(container, page, api, trash = false) {
  const count = page.total;
  const state = trash ? 'deleted' : 'archived';
  container.append(element('h2', 'Items'), element('p', count ? `${count} ${state} item${count === 1 ? '' : 's'}` : `No ${state} items`));
  if (!page.can_manage) container.append(element('p', 'Owner access required.'));
  if (page.has_more) container.append(element('p', `Showing the first ${page.items.length} items.`));
  const list = document.createElement('ul'); container.append(list);
  let busy = false;
  function start() {
    if (busy || !api.current()) return false;
    busy = true;
    for (const b of container.querySelectorAll('button')) b.disabled = true;
    return true;
  }
  function recover(error, message) {
    if (!api.current()) return;
    if (error === 401) return api.expired();
    if (error === 403) return api.refresh();
    container.append(element('p', message));
    const reload = element('button', 'Reload recovery list'); reload.onclick = api.refresh;
    container.append(reload); reload.focus();
  }
  async function submit(restore, send) {
    if (!start()) return;
    const label = restore.textContent;
    restore.textContent = 'Restoring…';
    try { await send(); }
    catch (error) {
      restore.textContent = label;
       return recover(error, error === 409 ? (trash ? 'Trash changed.' : 'Archive changed.') : 'Restore outcome unavailable.');
    }
    if (!api.current()) return;
    try { await api.readBoard(); if (api.current()) api.refresh(); }
    catch (error) { recover(error, 'Restore saved. Reload recovery list.'); }
  }
  for (const row of page.items) {
    const item = document.createElement('li'); item.dataset.itemId = row.id;
    const restore = element('button', 'Restore item'); restore.disabled = !row.restorable;
    item.append(element('span', `${row.title} · ${row.group_name}`), restore);
    if (!row.group_active) item.append(element('span', 'Restore its group first.'));
    list.append(item);
    restore.onclick = () => submit(restore, () => api.transition(row.id, trash ? 'restore-deleted' : 'restore', {operation_id: crypto.randomUUID(), board_revision: page.board_revision, item_revision: row.revision}));
  }
  if (trash) return;
  container.append(element('h2', 'Groups'));
  const groups = document.createElement('ul'); container.append(groups);
  if (!page.groups.length) container.append(element('p', 'No archived groups'));
  for (const group of page.groups) {
    const li = document.createElement('li'); li.dataset.groupId = group.id;
    const restore = element('button', 'Restore group'); restore.disabled = !page.can_manage;
    restore.onclick = () => submit(restore, () => api.transitionGroup(group.id, 'restore', page.board_revision));
    li.append(element('span', group.name), restore); groups.append(li);
  }
  if (page.next_group_offset !== null) {
    const more = element('button','Next groups'); container.append(more);
    more.onclick = async () => {
      if (!start()) return;
      try {
        const next = await api.more(page.board_revision, page.next_group_offset);
        if (!api.current()) return;
        container.replaceChildren(); mountArchive(container, next, api);
      } catch (error) { recover(error, 'Could not load next groups.'); }
    };
  }
}
