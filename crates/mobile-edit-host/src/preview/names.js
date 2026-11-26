// State belongs to one mounted board visit. Requests close over that visit's path;
// only its accepted rows supply editor baselines. A projection never changes them.
import {mountMoves} from './moves.js';
import {archiveDialog} from './lifecycle.js';
import {groupMenus} from './group-lifecycle.js';
import {groupOptions} from './group-create.js';
import {mountReorder} from './reorder.js';
import {mountSelection} from './selection.js';

export function mountNames(board, list, failureArea, api) {
  const failures = document.createElement('div'), moveFailures = document.createElement('div');
  const reorderStatus = document.createElement('div');
  reorderStatus.setAttribute('aria-live', 'polite');
  failureArea.replaceChildren(reorderStatus, failures, moveFailures);
  const rejected = new Map();
  const drafts = new Map(); // Group IDs are local to this board visit.
  let busy = false;
  const controls = {api, button, setBusy, busy: () => busy, board: () => board, refresh: next => {
      board = next;
      if (!board.editable) { drafts.clear(); rejected.clear(); }
      render(); showFailures();
    }};
  const moves = mountMoves({...controls, list, failures: moveFailures, archive: row => archiveDialog(row, controls, failureArea)});
  const selection = mountSelection(controls, failureArea);
  const grip = mountReorder({...controls, status: reorderStatus, multiDrag: selection.multiDrag});
  const notice = document.createElement('div'); notice.className = 'group-notice'; notice.setAttribute('role', 'status');
  failureArea.prepend(notice);
  const options = groupOptions(controls, document.querySelector('#new-item-options'), notice);
  const groupMenu = groupMenus(controls, notice, options.edit);

  function setBusy(value) {
    busy = value;
    for (const control of list.querySelectorAll('.add-item input, .add-item button')) {
      control.disabled = value || !board.editable;
    }
  }

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
    setBusy(true);
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
    } finally { if (api.current()) setBusy(false); }
  }

  async function save(id, edit) {
    setBusy(true);
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
      display.parentNode.querySelector('[type=checkbox]').setAttribute('aria-label', `Select ${accepted.title}`);
      display.parentNode.querySelector('.row-menu button').setAttribute('aria-label', `More options for ${accepted.title}`);
    } catch (error) {
      if (!api.current()) return;
      if (error === 422) {
        display.textContent = accepted.title;
        rejected.set(id, edit); showFailures();
      } else failed(error, error === 409 ? 'Name changed. Reload the board.' : 'Save outcome unavailable. Reload the board.');
    } finally { if (api.current()) setBusy(false); }
  }

  function cell(row, group) {
    const li = document.createElement('li');
    li.dataset.itemId = row.id;
    li.append(selection.checkbox(group, row));
    const menu = moves.menu(row, group);
    if (!board.editable) {
      const name = document.createElement('span'); name.className = 'item-name'; name.textContent = row.title;
      li.append(name, menu); return li;
    }
    const display = button(row.title, e => { if (e.detail === 0) open(); });
    display.className = 'item-name';
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
    if (Number.isSafeInteger(board.revision) && board.revision > 0) li.append(grip(row, group));
    li.append(display, menu); return li;
  }
  function addEntry(group) {
    const form = document.createElement('form');
    form.className = 'add-item';
    const input = document.createElement('input');
    input.setAttribute('aria-label', `Add item to ${group.name}`);
    const add = document.createElement('button');
    const status = document.createElement('span');
    status.setAttribute('role', 'status');
    const draft = drafts.get(group.id) ?? {id: crypto.randomUUID(), title: '', rejected: false};
    drafts.set(group.id, draft);
    input.value = draft.title;
    input.oninput = () => { draft.title = input.value; };
    function present() {
      add.textContent = draft.rejected ? 'Retry' : '+ Add item';
      status.textContent = draft.rejected ? 'Could not add item.' : '';
      input.disabled = add.disabled = busy || !board.editable;
    }
    input.onkeydown = e => {
      if (e.key === 'Escape' && !busy) {
        draft.title = input.value = ''; draft.id = crypto.randomUUID(); draft.rejected = false;
        present();
      }
      if (e.key === 'Enter' && e.isComposing) e.preventDefault();
    };
    form.onsubmit = async event => {
      event.preventDefault();
      if (busy || !board.editable || !api.current() || !input.value.trim()) return;
      setBusy(true);
      status.textContent = 'Saving…';
      try {
        const saved = await api.create({id: draft.id, group_id: group.id, title: input.value});
        if (!api.current()) return;
        if (saved.id !== draft.id || saved.group_id !== group.id || typeof saved.title !== 'string' ||
            !Number.isSafeInteger(saved.revision) || saved.revision < 1) throw Error('Unexpected created item');
      } catch (error) {
        if (!api.current()) return;
        if (error === 422) {
          draft.rejected = true; setBusy(false); present(); input.focus();
        } else if (error === 403) api.denied('Item creation denied.');
        else failed(error, 'Creation outcome unavailable. Reload the board.');
        return;
      }
      drafts.delete(group.id); input.value = '';
      try {
        const refreshed = await api.read();
        if (!api.current()) return;
        board = refreshed;
        if (!board.editable) { drafts.clear(); rejected.clear(); }
        setBusy(false); render(); showFailures();
        list.querySelector(`[data-group-id="${CSS.escape(group.id)}"] .add-item input`)?.focus();
      } catch (error) {
        if (api.current()) failed(error, 'Item added. Reload the board.');
      }
    };
    present(); form.append(input, add, status); return form;
  }

  function section(group) {
    const region = document.createElement('section');
    region.dataset.groupId = group.id;
    const heading = document.createElement('h2');
    const count = group.items.length;
    const name = document.createElement('span'); name.textContent = group.name;
    if (board.owner) name.ondblclick = () => options.edit(group, region.querySelector('.group-menu button'));
    heading.append(name, ` / ${count ? count + (count === 1 ? ' item' : ' items') : 'No Items'}`);
    heading.id = `group-${group.id}`;
    heading.style.borderLeft = `0.3rem solid ${group.colour}`;
    region.setAttribute('aria-labelledby', heading.id);
    const rows = document.createElement('ul');
    rows.setAttribute('aria-label', 'Board items');
    rows.append(...group.items.map(row => cell(row, group)));
    region.append(selection.checkbox(group), groupMenu(group), heading, rows, addEntry(group));
    return region;
  }
  function render() {
    options.create.disabled = !board.owner;
    selection.resetSnapshot(); list.replaceChildren(...board.groups.map(section));
    selection.paint(); moves.showFailures();
  }
  render();
}
