// Both entry points edit the same fields. Only creation has a replay identity.
export function groupOptions({api, board, busy, setBusy, refresh, button}, host, notice) {
  const menu = document.createElement('details'), toggle = document.createElement('summary');
  toggle.textContent = 'New item options';
  const create = button('New group of items', () => open(null, toggle));
  menu.append(toggle, create); host.append(menu);
  return {create, edit:open};

  function open(group, opener) {
    if (busy() || !board().owner || !api.current()) return;
    menu.open = false; setBusy(true);
    const id = group?.id ?? `g-${crypto.randomUUID()}`;
    let groupRevision = group?.revision;
    let baseline = board().revision, latest, phase = 'draft', pending = false, uncertain = false;
    const dialog = document.createElement('dialog'), heading = document.createElement('h2');
    heading.textContent = group ? 'Edit group' : 'Add group'; dialog.setAttribute('aria-label', heading.textContent);
    const name = document.createElement('input'), colour = document.createElement('select');
    function label(text, field) {
      const el = document.createElement('label'); el.append(text, field); return el;
    }
    name.value = group?.name ?? 'New group'; name.autofocus = true;
    colour.add(new Option('Blue', '#579bfc')); colour.add(new Option('Green', '#00c875'));
    if (group) {
      if (![...colour.options].some(o => o.value === group.colour)) colour.add(new Option(group.colour, group.colour));
      colour.value = group.colour;
    }
    const status = document.createElement('p'); status.setAttribute('role','status');
    const save = button('Save group', submit), cancel = button('Cancel', close);
    const review = button('Review latest board', () => readLatest(true));
    const adopt = button('Keep my draft and use this revision', () => {
      if (group) groupRevision = latest.groups.find(g => g.id === id).revision;
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
      dialog.close(); dialog.remove(); setBusy(false); opener.focus();
    }
    function denied(error) {
      if (error === 401) api.expired();
      else if (error === 403) api.denied(group ? 'Group edit denied.' : 'Group creation denied.');
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
      const present = latest.groups.find(g => g.id === id);
      if (group) {
        const accepted = present ? `Board revision ${latest.revision}, group ${id} revision ${present.revision}: ${present.name} / ${present.colour}` : `Group ${id} is no longer available.`;
        if (forReview && present && latest.owner) {
          phase = 'reviewed'; status.textContent = accepted; paint(); adopt.focus();
        } else {
          finish(); refresh(latest); notice.textContent = accepted;
        }
        return;
      }
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
        const fields = {board_revision:baseline, name:name.value, colour:colour.value};
        const ack = group ? await api.editGroup(id, {...fields, group_revision:groupRevision}) : await api.createGroup({...fields, id});
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
        if (group) {
          phase = 'reload'; status.textContent = error === 404 ? 'Group unavailable. Reload the board.' : 'Save outcome unavailable. Reload the board.';
          paint(); reload.focus(); return;
        }
        // A proposed create ID, unlike an edit target, can identify a replay.
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
