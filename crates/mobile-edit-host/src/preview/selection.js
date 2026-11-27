// One complete, bounded physical grid. Never infer group membership from controls.
import {openSelectionMove} from './selection-move.js';
import {archiveDialog} from './lifecycle.js';

export function mountSelection(controls, area) {
  const {api, board, busy, setBusy, button} = controls;
  const checked = new Set(), rowControls = new Map(), headers = new Map();
  let anchor, resolved, snapshot;
  const toolbar = document.createElement('div'), count = document.createElement('span');
  toolbar.setAttribute('role', 'toolbar'); toolbar.setAttribute('aria-label', 'Selected item actions');
  const notice = document.createElement('div'); notice.setAttribute('role', 'status');
  area.prepend(notice);
  toolbar.append(count, ' ', button('Clear item selection', () => {
    if (!busy() && api.current()) { checked.clear(); notice.replaceChildren(); paint(); }
  }));
  const move = button('Move to', () => openSelectionMove(controls, area, checked));
  const archive = button('Archive', () => archiveDialog([...checked].map(id => board().rows.find(r => r.id === id)), controls, area, () => checked.clear()));
  archive.className = 'archive-selected';
  const remove = button('Delete', () => archiveDialog([...checked].map(id => board().rows.find(r => r.id === id)), controls, area, () => checked.clear(), true));
  remove.className = 'delete-selected';
  toolbar.append(archive, remove, move);

  function paint() {
    move.disabled = !board().editable;
    archive.disabled = !board().owner;
    remove.disabled = !board().owner;
    for (const [id, box] of rowControls) {
      box.checked = checked.has(id);
      box.closest('li').classList.toggle('selected-item', box.checked);
    }
    for (const group of board().groups) {
      const box = headers.get(group.id), n = group.items.filter(r => checked.has(r.id)).length;
      if (!box) continue;
      box.checked = n > 0 && n === group.items.length;
      box.indeterminate = n > 0 && !box.checked;
    }
    if (checked.size) { count.textContent = `${checked.size} selected`; if (!toolbar.isConnected) area.prepend(toolbar); }
    else toolbar.remove();
  }

  function apply(ids, add) {
    for (const id of ids) { checked.delete(id); if (add) checked.add(id); }
    paint();
  }
  const layout = groups => groups.map(g => [g.id, g.items.map(r => r.id)]);

  async function toggle(group, row, shift, selectAll = false) {
    // Native checkbox activation has already flipped the target; show only accepted state.
    paint();
    if (busy() || !api.current()) return;
    notice.replaceChildren();
    if (row && (!shift || !anchor)) {
      anchor = {group: group.id, id: row.id};
      apply([row.id], !checked.has(row.id)); return;
    }
    setBusy(true);
    try {
      if (!resolved) {
        const fresh = await api.read();
        if (!api.current()) return;
        const ordered = layout(fresh.groups);
        if (JSON.stringify(ordered) !== JSON.stringify(layout(board().groups))) throw Error('Layout changed');
        resolved = new Map(ordered);
      }
      const members = resolved.get(group.id);
      if (!row) { apply(members, selectAll || !members.every(id => checked.has(id))); return; }
      if (!resolved.get(anchor.group)?.includes(anchor.id) || !members.includes(row.id)) throw Error('Range unavailable');
      const order = Array.from(resolved.values()).flat();
      const a = order.indexOf(anchor.id), b = order.indexOf(row.id);
      apply(order.slice(Math.min(a,b), Math.max(a,b)+1), !checked.has(row.id));
    } catch (error) {
      if (!api.current()) return;
      resolved = undefined;
      if (error === 401) api.expired();
      else if (error === 403 || error === 404) api.reload('Destination unavailable');
      else if (error === 413) api.reload('Board too large for this preview');
      else {
        notice.textContent = 'Could not resolve selection. Try the selection again or ';
        notice.append(button('Reload board', api.reopen));
      }
    } finally { if (api.current()) setBusy(false); }
  }
  function checkbox(group, row) {
    const box = document.createElement('input'); box.type = 'checkbox';
    box.setAttribute('aria-label', row ? `Select ${row.title}` : `Select all items in ${group.name}`);
    box.disabled = !row && !group.items.length;
    box.onclick = event => toggle(group, row, event.shiftKey);
    if (row) rowControls.set(row.id, box); else headers.set(group.id, box);
    return box;
  }
  function resetSnapshot() {
    rowControls.clear(); headers.clear();
    // A Viewer folding a section remounts controls, not the accepted grid.
    if (snapshot === board()) return;
    snapshot = board(); resolved = undefined; notice.replaceChildren();
    const visible = new Set(board().rows.map(r => r.id));
    for (const id of checked) if (!visible.has(id)) checked.delete(id);
  }
  return {checkbox, paint, resetSnapshot, selectGroup: group => toggle(group, null, false, true), multiDrag: id => checked.size > 1 && checked.has(id)};
}
