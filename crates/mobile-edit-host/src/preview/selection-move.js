// A dialog owns one command identity and an ordered, immutable list of roots.
export function openSelectionMove({api, board, refresh, button, busy, setBusy}, area, checked) {
  if (busy() || !api.current() || !board().editable || !checked.size) return;
  const ids = [...checked], rows = ids.map(id => board().rows.find(r => r.id === id));
  let boardRevision = board().revision, revisions = rows.map(r => r.revision);
  const operation = crypto.randomUUID();
  const dialog = document.createElement('dialog'), heading = document.createElement('h2');
  heading.textContent = 'Move selected items'; dialog.setAttribute('aria-label', heading.textContent);
  const summary = document.createElement('p');
  summary.textContent = `${ids.length} selected: ${rows.slice(0,6).map(r=>r.title).join(', ')}` +
    (ids.length > 6 ? ` and ${ids.length-6} more` : '');
  function choice(name, options) {
    const label = document.createElement('label'), select = document.createElement('select');
    label.textContent = name; label.append(select);
    for (const [id,text] of options) select.add(new Option(text,id));
    dialog.append(label); return select;
  }
  dialog.append(heading,summary);
  choice('Destination board', [[board().id,'Current board']]).disabled = true;
  const group = choice('Destination group', board().groups.map(g=>[g.id,g.name]));
  const status = document.createElement('p'), detail = document.createElement('div');
  status.setAttribute('role','status');
  const confirm = button('Confirm', submit), cancel = button('Cancel', () => dialog.close());
  const review = button('Review latest items', reviewLatest);
  let reviewed, needsReview = false;
  const adopt = button('Use these revisions and keep my choices', () => {
    if (!reviewed || busy() || !api.current()) return;
    boardRevision = reviewed.revision; revisions = reviewed.revisions;
    needsReview = false; resetReview(); status.textContent = 'Revisions updated. Confirm to move.'; pending(false);
  });
  function resetReview() {
    reviewed = undefined; detail.replaceChildren(); adopt.hidden = review.hidden = true;
  }
  function pending(value) {
    setBusy(value); group.disabled = cancel.disabled = review.disabled = value;
    confirm.disabled = value || needsReview; adopt.disabled = value || !reviewed;
  }
  function readError(error) {
    if (error === 401) api.expired();
    else api.reload(error === 403 || error === 404 ? 'Destination unavailable' :
      error === 413 ? 'Board too large for this preview' : 'Could not confirm board. Reload the board.');
  }
  async function reread(clear) {
    try {
      const fresh = await api.read();
      if (!api.current()) return;
      if (clear) checked.clear();
      dialog.remove(); refresh(fresh);
    } catch (e) { if (api.current()) readError(e); }
  }
  async function reviewLatest() {
    if (busy() || !api.current()) return;
    pending(true);
    try {
      const fresh = await api.read();
      if (!api.current()) return;
      if (!fresh.editable) { dialog.remove(); refresh(fresh); return; }
      const current = ids.map(id => fresh.rows.find(r=>r.id===id));
      detail.replaceChildren(...current.map((r,i) => {
        const p = document.createElement('p'); p.textContent = r ? `${r.title} (${ids[i]}), revision ${r.revision}` : `${ids[i]}: Unavailable`;
        return p;
      }));
      needsReview = true;
      reviewed = current.every(Boolean) && fresh.groups.some(g=>g.id===group.value) &&
        {revision:fresh.revision, revisions:current.map(r=>r.revision)};
      adopt.hidden = false; status.textContent = 'Review latest items';
    } catch (e) { if (api.current()) readError(e); }
    finally { if (api.current()) pending(false); }
  }
  async function submit() {
    if (busy() || needsReview || !api.current()) return;
    resetReview(); pending(true); status.textContent = 'Saving…';
    try {
      const ack = await api.moveSelection({operation_id:operation, group_id:group.value, board_revision:boardRevision,
        items:ids.map((id,i)=>({id,revision:revisions[i]}))});
      if (!api.current()) return;
      if (ack.operation_id !== operation || ack.board_id !== board().id || !Array.isArray(ack.affected_ids) ||
          ack.affected_ids.length !== ids.length || new Set(ack.affected_ids).size !== ids.length ||
          !ack.affected_ids.every(id=>checked.has(id))) throw Error('Unexpected acknowledgment');
      await reread(true);
    } catch (e) {
      if (!api.current()) return;
      if (e === 403) await reread(false);
      else if ([404,409,422,500].includes(e)) {
        needsReview = e !== 500;
        status.textContent = needsReview ? 'Selection changed. Review latest items.' : 'Could not save move.';
        review.hidden = false;
      } else readError(e);
    } finally { if (api.current()) pending(false); }
  }
  group.onchange = () => { if (!adopt.hidden) { resetReview(); review.hidden = false; } };
  resetReview(); dialog.oncancel = e => { if (busy()) e.preventDefault(); };
  dialog.onclose = () => dialog.remove();
  dialog.append(confirm,cancel,status,review,detail,adopt); area.append(dialog); dialog.showModal(); group.focus();
}
