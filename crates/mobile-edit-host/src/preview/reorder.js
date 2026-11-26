// Pointer capture keeps the release (including its click) on the grip. Hit-test
// the actual pointer coordinates for the anchor; never rearrange DOM rows locally.
export function mountReorder({api, board, refresh, busy, setBusy, status, button, multiDrag}) {
  function unavailable(error) {
    if (error === 401) api.expired();
    else if (error === 403) api.denied('Item reorder denied.');
    else return false;
    return true;
  }
  async function read() {
    try {
      const next = await api.read();
      if (api.current()) { refresh(next); status.replaceChildren(); }
    } catch (error) {
      if (api.current() && !unavailable(error)) api.reload('Could not refresh order. Reload the board.');
    }
  }
  async function drop(item, draft) {
    setBusy(true); status.textContent = 'Saving order…';
    try {
      const ack = await api.reorder(item, draft);
      if (!api.current()) return;
      if (ack.operation_id !== draft.operation_id || ack.board_id !== board().id || ack.item_id !== item) throw Error('Unknown acknowledgment');
      await read();
    } catch (error) {
      if (!api.current() || unavailable(error)) return;
      status.textContent = error === 409 ? 'Order changed. ' : 'Could not confirm order. ';
      status.append(button('Reload board', async () => {
        if (busy() || !api.current()) return;
        setBusy(true); await read(); if (api.current()) setBusy(false);
      }));
    } finally { if (api.current()) setBusy(false); }
  }
  return (row, group) => {
    const grip = document.createElement('span');
    grip.className = 'reorder-grip'; grip.tabIndex = -1;
    grip.title = 'Drag to reorder within group'; grip.textContent = '⠿';
    let start, target, dragging;
    function clear() { if (target) delete target.dataset.dropEdge; target = undefined; }
    function cancel() { clear(); start = undefined; dragging = false; }
    grip.onpointerdown = e => {
      if (e.button !== 0 || !e.isPrimary || e.pointerType === 'touch' || busy() || !api.current()) return;
      grip.focus(); // Blur can start a pending name save.
      if (busy() || multiDrag(row.id)) return;
      e.preventDefault(); grip.setPointerCapture(e.pointerId);
      start = {x:e.clientX, y:e.clientY, pointer:e.pointerId, board:board().revision, item:row.revision};
    };
    function mark(e) {
      clear();
      if (!start || busy() || multiDrag(row.id) || !api.current()) return;
      dragging ||= Math.hypot(e.clientX-start.x, e.clientY-start.y) >= 6;
      if (!dragging) return;
      const li = document.elementFromPoint(e.clientX,e.clientY)?.closest('li[data-item-id]');
      if (!li || li.parentElement !== grip.closest('li').parentElement || li.dataset.itemId === row.id) return;
      const rect = li.getBoundingClientRect();
      target = li; target.dataset.dropEdge = e.clientY < rect.top+rect.height/2 ? 'before' : 'after';
    }
    grip.onpointermove = e => { if (start?.pointer === e.pointerId) mark(e); };
    grip.onpointerup = e => {
      if (start?.pointer !== e.pointerId) return;
      mark(e);
      const draft = target && {operation_id:crypto.randomUUID(), board_revision:start.board, item_revision:start.item,
        group_id:group.id, anchor_id:target.dataset.itemId, side:target.dataset.dropEdge};
      cancel(); if (draft) drop(row.id, draft);
    };
    grip.onkeydown = e => { if (e.key === 'Escape') { cancel(); e.preventDefault(); } };
    grip.onpointercancel = grip.onlostpointercapture = grip.onblur = cancel;
    grip.onclick = e => { e.preventDefault(); e.stopPropagation(); };
    return grip;
  };
}
