// The dialog owns the proposed identity, raw spelling and revision until close.
export function groupOptions({api, board, busy, setBusy, refresh, button}, host, notice) {
  const menu = document.createElement('details'), toggle = document.createElement('summary');
  toggle.textContent = 'New item options';
  const create = button('New group of items', open);
  menu.append(toggle, create); host.append(menu);
  return create;

  function open() {
    if (busy() || !board().owner || !api.current()) return;
    menu.open = false; setBusy(true);
    const id = `g-${crypto.randomUUID()}`;
    let baseline = board().revision, latest, phase = 'draft', pending = false, uncertain = false;
    const dialog = document.createElement('dialog'), heading = document.createElement('h2');
    heading.textContent = 'Add group'; dialog.setAttribute('aria-label', heading.textContent);
    const name = document.createElement('input'), colour = document.createElement('select');
    function label(text, field) {
      const el = document.createElement('label'); el.append(text, field); return el;
    }
    name.value = 'New group'; name.autofocus = true;
    colour.add(new Option('Blue', '#579bfc')); colour.add(new Option('Green', '#00c875'));
    const status = document.createElement('p'); status.setAttribute('role','status');
    const save = button('Save group', submit), cancel = button('Cancel', close);
    const review = button('Review latest board', () => readLatest(true));
    const adopt = button('Keep my draft and use this revision', () => {
      baseline = latest.revision; phase = 'draft'; status.textContent = ''; paint(); name.focus();
    });
    const reload = button('Reload board', () => readLatest(false));
    function paint() {
      name.disabled = colour.disabled = pending || phase === 'reload';
      save.disabled = pending || phase !== 'draft' || !name.value.trim();
      save.hidden = phase === 'reload'; cancel.disabled = pending || phase === 'reload';
      review.hidden = phase !== 'conflict'; adopt.hidden = phase !== 'reviewed';
      reload.hidden = phase !== 'reload'; review.disabled = reload.disabled = pending;
    }
    function close() {
      if (pending || phase === 'reload') return;
      finish();
    }
    function finish() {
      dialog.close(); dialog.remove(); setBusy(false); toggle.focus();
    }
    function denied(error) {
      if (error === 401) api.expired();
      else if (error === 403) api.denied('Group creation denied.');
      else return false;
      return true;
    }
    async function readLatest(forReview) {
      pending = true; paint();
      try {
        latest = await api.read();
      } catch (error) {
        if (!api.current() || denied(error)) return;
        pending = false;
        status.textContent = 'Could not read the latest board.'; paint(); return;
      }
      if (!api.current()) return;
      pending = false;
      const present = latest.groups.some(g => g.id === id);
      if (!present && latest.owner && (forReview || uncertain)) {
        uncertain = false;
        if (forReview || latest.revision !== baseline) {
          phase = 'reviewed';
          status.textContent = `Board revision ${latest.revision}: ${latest.groups.map(g => g.name).join(', ')}`;
          paint(); adopt.focus();
        } else {
          phase = 'draft'; status.textContent = 'Group not found. Check the draft before saving again.';
          paint(); name.focus();
        }
        return;
      }
      finish(); refresh(latest);
      if (!forReview && !present) notice.textContent = 'The proposed group is not on the active board.';
    }
    async function submit() {
      if (pending || phase !== 'draft' || !name.value.trim() || !api.current()) return;
      pending = true; status.textContent = 'Saving…'; paint();
      try {
        const ack = await api.createGroup({id, board_revision:baseline, name:name.value, colour:colour.value});
        if (ack.board_id !== board().id || ack.group_id !== id) throw Error('Unexpected acknowledgment');
      } catch (error) {
        if (!api.current() || denied(error)) return;
        pending = false;
        if (error === 409) {
          phase = 'conflict'; status.textContent = 'Board changed.'; paint(); review.focus(); return;
        }
        if ([422,413,500].includes(error)) {
          status.textContent = error === 413 ? 'Board too large for this preview.' : 'Could not save group.';
          paint(); name.focus(); return;
        }
        // The response may have been lost after commit. Resolve by ID, never by another write.
        uncertain = true;
      }
      if (!api.current()) return;
      phase = 'reload'; await readLatest(false);
    }
    name.oninput = paint;
    name.onkeydown = e => { if (e.key === 'Enter' && !e.isComposing) { e.preventDefault(); submit(); } };
    dialog.oncancel = e => { e.preventDefault(); close(); };
    dialog.append(heading, label('Group name',name), label('Group colour',colour), status, save, review, adopt, reload, cancel);
    host.append(dialog); paint(); dialog.showModal(); name.focus(); name.setSelectionRange(name.value.length,name.value.length);
  }
}
