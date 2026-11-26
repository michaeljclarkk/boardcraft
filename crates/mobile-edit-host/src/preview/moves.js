// Mounted for one selected-board visit. Only Confirm and Apply call the PUT.
export function mountMoves({api, list, failures, button, board, refresh, busy, setBusy, archive}) {
  const failed = new Map();
  const sourceOf = (state, id) => state.groups.find(g => g.items.some(r => r.id === id));
  function accessError(error) {
    if (error === 401) api.expired();
    else if (error === 403) api.denied('Item move denied.');
    else return false;
    return true;
  }
  function showFailures() {
    failures.replaceChildren();
    for (const [id, edit] of failed) {
      const note = document.createElement('p');
      note.setAttribute('role', 'alert'); note.textContent = edit.message;
      failures.append(note);
      if (edit.review) {
        for (const [label, group] of [['Current group', edit.review.source], ['Requested group', edit.review.destination]]) {
          const detail = document.createElement('p');
          detail.dataset.groupId = group?.id ?? '';
          detail.textContent = `${label}: ${group ? `${group.name} (${group.id})` : 'Unavailable'}`;
          failures.append(detail);
        }
      }
      failures.append(button('Review move', () => review(edit)));
      if (edit.review?.revision && available(edit)) failures.append(button('Apply reviewed move', () => {
        if (!busy() && api.current() && available(edit)) submit(edit, edit.review.revision);
      }));
      failures.append(button('Dismiss', () => {
        if (!busy()) { failed.delete(id); showFailures(); }
      }));
    }
  }
  function available(edit) {
    return board().editable && sourceOf(board(), edit.id) && board().groups.some(g => g.id === edit.to);
  }
  function remember(edit, message) {
    delete edit.review;
    edit.message = message; failed.set(edit.id, edit); showFailures();
  }
  async function latest() {
    const state = await api.read();
    if (!api.current()) return;
    refresh(state);
    return state;
  }
  async function review(edit) {
    if (busy() || !api.current()) return;
    setBusy(true);
    try {
      const state = await latest();
      if (!state) return;
      const source = sourceOf(state, edit.id), destination = state.groups.find(g => g.id === edit.to);
      const row = source?.items.find(r => r.id === edit.id);
      edit.review = {source, destination, revision: state.editable && destination && row?.revision};
      showFailures();
    } catch (error) {
      if (api.current() && !accessError(error)) remember(edit, 'Could not review move.');
    } finally { if (api.current()) setBusy(false); }
  }
  async function submit(edit, revision, dialog) {
    setBusy(true);
    if (dialog) {
      for (const control of dialog.querySelectorAll('button, select')) control.disabled = true;
      dialog.querySelector('[role=status]').textContent = 'Saving…';
    } else remember(edit, 'Saving…');
    try {
      const saved = await api.move(edit.id, edit.to, revision);
      if (!api.current()) return;
      if (saved.id !== edit.id || saved.group_id !== edit.to || typeof saved.title !== 'string' ||
          !Number.isSafeInteger(saved.revision) || saved.revision < revision) throw Error('Invalid move result');
    } catch (error) {
      if (api.current() && !accessError(error)) {
        remember(edit, error === 409 ? 'Group changed. Review move.' : error === 422 ? 'Destination group unavailable.' : 'Could not confirm move.');
      }
      return;
    } finally {
      dialog?.remove();
      if (api.current()) setBusy(false);
    }
    failed.delete(edit.id); showFailures(); setBusy(true);
    try { await latest(); }
    catch (error) { if (api.current() && !accessError(error)) api.reload('Move saved. Reload the board.'); }
    finally { if (api.current()) setBusy(false); }
  }
  async function open(row, displayedGroup) {
    if (busy() || !api.current()) return;
    setBusy(true);
    try {
      const state = await latest();
      if (!state) return;
      const source = sourceOf(state, row.id);
      if (!source || !state.editable) { api.denied('Item move unavailable.'); return; }
      const revision = source.items.find(r => r.id === row.id).revision;
      const dialog = document.createElement('dialog');
      dialog.setAttribute('aria-label', 'Move selected items');
      const heading = document.createElement('h2'); heading.textContent = 'Move selected items';
      const boardLabel = document.createElement('label'); boardLabel.textContent = 'Destination board';
      const fixed = document.createElement('select'); fixed.disabled = true;
      fixed.add(new Option('Current board', state.id)); boardLabel.append(fixed);
      const label = document.createElement('label'); label.textContent = 'Destination group';
      const select = document.createElement('select');
      for (const g of state.groups) select.add(new Option(g.name, g.id));
      label.append(select);
      const status = document.createElement('span'); status.setAttribute('role','status');
      const confirm = button('Confirm', () => {
        if (busy() || !api.current()) return;
        const edit = {id: row.id, to: select.value};
        if (source.id !== displayedGroup.id) {
          dialog.remove(); remember(edit, 'Group changed. Review move.');
        } else submit(edit, revision, dialog);
      });
      dialog.oncancel = event => { if (busy()) event.preventDefault(); };
      dialog.onclose = () => dialog.remove();
      dialog.append(heading, boardLabel, label, confirm, button('Cancel', () => dialog.close()), status);
      list.append(dialog); dialog.showModal(); select.focus();
    } catch (error) { if (api.current() && !accessError(error)) api.reload('Could not open move. Reload the board.'); }
    finally { if (api.current()) setBusy(false); }
  }
  function menu(row, group) {
    const container = document.createElement('span'); container.className = 'row-menu';
    const more = button('⋯', () => {
      if (busy() || !api.current()) return;
      lifecycle.hidden = move.hidden = !move.hidden; more.setAttribute('aria-expanded', String(!move.hidden));
    });
    more.setAttribute('aria-label', `More options for ${row.title}`); more.setAttribute('aria-expanded','false');
    const move = button('Move to', () => open(row, group)); move.disabled = !board().editable; move.hidden = true;
    const lifecycle = button('Archive', () => archive(row)); lifecycle.className = 'archive-item';
    lifecycle.hidden = true; lifecycle.disabled = !board().owner;
    if (lifecycle.disabled) lifecycle.title = 'Owner access required.';
    container.append(more, lifecycle, move); return container;
  }
  return {menu, showFailures};
}
