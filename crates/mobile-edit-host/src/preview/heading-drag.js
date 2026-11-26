// The table owns one pointer for this visit. Capture routes events, not hit tests:
// choose the target from the current coordinates even while the viewport scrolls.
export function mountHeadingDrag(list, {board, busy, api}, move) {
  const listeners = new AbortController();
  const on = (el, type, fn, capture = false) => el.addEventListener(type, fn, {capture, signal: listeners.signal});
  let drag, label, marker, frame, suppressUntil = 0, swallowedClick = false;

  function clearMarker() {
    if (marker) delete marker.dataset.dropEdge;
    marker = undefined;
  }
  function finish() {
    cancelAnimationFrame(frame); frame = undefined;
    clearMarker();
    if (label) {
      label.remove(); label = undefined;
      suppressUntil = performance.now() + 500;
    }
    if (drag?.name.hasPointerCapture(drag.pointer)) drag.name.releasePointerCapture(drag.pointer);
    drag = undefined;
  }
  function available() { return api.current() && !busy() && board().owner && board().editable; }
  function mark() {
    clearMarker();
    const {x,y,id} = drag, bounds = list.getBoundingClientRect();
    const top = Math.max(0,bounds.top), bottom = Math.min(innerHeight,bounds.bottom);
    if (x < bounds.left || x >= bounds.right || y < top || y >= bottom) return 0;
    const region = document.elementFromPoint(x,y)?.closest('section[data-group-id]');
    if (region?.parentNode === list && region.dataset.groupId !== id) {
      marker = region.querySelector('h2');
      const rect = marker.getBoundingClientRect();
      marker.dataset.dropEdge = y <= rect.top + rect.height/2 ? 'before' : 'after';
    }
    const edge = Math.min(44, (bottom-top)/2);
    if (y < top+edge && list.scrollTop > 0) return -Math.ceil((top+edge-y)/3);
    if (y > bottom-edge && list.scrollTop+list.clientHeight < list.scrollHeight) return Math.ceil((y-bottom+edge)/3);
    return 0;
  }
  function track() {
    cancelAnimationFrame(frame); frame = undefined;
    if (!available() || !drag.name.isConnected) return finish();
    const speed = mark();
    if (speed) frame = requestAnimationFrame(() => { list.scrollTop += speed; track(); });
  }
  function update(e) {
    if (!available()) return finish();
    drag.x = e.clientX; drag.y = e.clientY;
    if (!label) {
      if (Math.hypot(drag.x-drag.startX, drag.y-drag.startY) < 6) return;
      label = document.createElement('span'); label.className = 'heading-drag-label';
      label.setAttribute('role', 'status'); label.textContent = drag.name.textContent;
      document.body.append(label);
    }
    label.style.left = `${drag.x+8}px`; label.style.top = `${drag.y+8}px`;
    track();
  }
  on(list,'pointerdown', e => {
    if (drag || !e.isPrimary || e.button !== 0 || e.pointerType === 'touch') return;
    swallowedClick = false;
    const name = e.target;
    if (!name.matches('section[data-group-id] > h2 > span') || !available()) return;
    drag = {name, id:name.closest('section').dataset.groupId, pointer:e.pointerId, startX:e.clientX, startY:e.clientY};
    name.setPointerCapture(e.pointerId);
  });
  on(list,'pointermove', e => { if (drag?.pointer === e.pointerId) update(e); });
  on(list,'pointerup', e => {
    if (drag?.pointer !== e.pointerId) return;
    update(e);
    let next;
    if (marker) {
      const ids = board().groups.map(g => g.id);
      next = ids.filter(id => id !== drag.id);
      const at = next.indexOf(marker.parentElement.dataset.groupId);
      next.splice(at + (marker.dataset.dropEdge === 'after' ? 1 : 0), 0, drag.id);
      if (next.every((id,i) => id === ids[i])) next = undefined;
    }
    finish(); if (next) move(next);
  });
  for (const type of ['pointercancel','lostpointercapture']) on(list,type,e => {
    if (drag?.pointer === e.pointerId) finish();
  });
  on(window,'keydown',e => { if (drag && e.key === 'Escape') { finish(); e.preventDefault(); } });
  on(list,'click',e => {
    if (performance.now() >= suppressUntil) return;
    suppressUntil = 0; swallowedClick = true;
    e.preventDefault(); e.stopImmediatePropagation();
  },true);
  on(list,'dblclick',e => { if (swallowedClick) { e.preventDefault(); e.stopImmediatePropagation(); } },true);
  return () => { finish(); listeners.abort(); };
}
