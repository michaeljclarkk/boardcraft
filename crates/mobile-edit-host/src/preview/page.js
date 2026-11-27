import {mountNames} from './names.js';
import {mountArchive} from './lifecycle.js';
import {readBinding, changeBinding, onChange} from './binding.js';

const $ = selector => document.querySelector(selector);
const identify = $('#identify'), authenticate = $('#authenticate'), boards = $('#boards');
const identifier = $('#identifier'), password = $('#password'), alert = $('[role=alert]');
const refresh = $('#refresh'), resolve = $('#resolve'), list = $('#boards ul');
const destination = $('#destination'), items = $('#items'), retry = $('#retry-destination');
let binding, active, visit, unmount;

function leaveVisit() {
  visit = undefined;
  unmount?.(); unmount = undefined;
}

function step(panel, title) {
  for (const view of [identify, authenticate, boards, destination, resolve]) view.hidden = view !== panel;
  $('#heading').textContent = title;
  alert.textContent = '';
}

function clearNavigation() {
  leaveVisit();
  $('#failures').replaceChildren();
  list.replaceChildren(); $('#empty').hidden = true;
  items.replaceChildren(); retry.hidden = true; retry.onclick = null;
  $('#recovery-nav').replaceChildren();
  $('#new-item-options').replaceChildren();
}

function signInScreen() {
  binding = undefined;
  identify.reset(); authenticate.reset();
  clearNavigation();
  step(identify, 'Sign in');
}

// History traversal can start a new read while an older one is still in flight.
async function run(operation) {
  leaveVisit(); // Invalidate editor blur and late saves before changing focus.
  const turn = active = Symbol();
  const current = () => active === turn;
  const controls = document.querySelectorAll('input, button');
  for (const control of controls) control.disabled = true;
  alert.textContent = '';
  try { await operation(current); }
  finally {
    if (current()) {
      active = undefined;
      for (const control of controls) control.disabled = false;
      if (!identify.hidden) identifier.focus();
      else if (!authenticate.hidden) password.focus();
      else if (!destination.hidden) (retry.hidden ? $('#back') : retry).focus();
      else if (!boards.hidden) (list.querySelector('a') ?? refresh).focus();
    }
  }
}

async function fetchResponse(path, options = {}) {
  const response = await fetch('/preview/api' + path, {
    mode: 'same-origin', credentials: 'same-origin', redirect: 'error', cache: 'no-store',
    signal: AbortSignal.timeout(15000),
    headers: {'Boardcraft-Preview': binding ?? 'login', 'Content-Type': 'application/json'},
    ...options
  });
  if (!response.ok) throw response.status;
  return response;
}

async function request(path, options) { return (await fetchResponse(path, options)).json(); }

function navigate(path) {
  history.pushState(null, '', path);
  run(loadRoute);
}

function loadRoute(current) {
  const archive = /^\/boards\/([^/]+)\/archive$/.exec(location.pathname);
  if (archive) return loadDestination(archive[1], current, '', true);
  return location.pathname === '/preview/' ? loadBoards(current)
    : loadDestination(location.pathname.slice('/boards/'.length), current);
}

window.addEventListener('popstate', () => {
  // Sign-in and session resolution use the current address when they finish.
  if (!boards.hidden || !destination.hidden) run(loadRoute);
});

async function loadBoards(current) {
  clearNavigation();
  step(boards, 'Available boards');
  try {
    const rows = await request('/navigation/boards');
    if (!current()) return;
    const items = rows.map(({id, name}) => {
      const item = document.createElement('li');
      const link = document.createElement('a');
      link.href = `/boards/${encodeURIComponent(id)}`;
      link.dataset.boardId = id; link.textContent = name;
      link.onclick = event => {
        if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
        event.preventDefault();
        if (!active) navigate(link.pathname);
      };
      item.append(link);
      return item;
    });
    list.replaceChildren(...items);
    $('#empty').hidden = items.length !== 0;
    refresh.textContent = 'Refresh';
  } catch (error) {
    if (!current()) return;
    if (error === 401) return forgetSession();
    refresh.textContent = 'Retry board list';
    alert.textContent = 'Could not load boards';
  }
}

async function loadDestination(id, current, notice = '', recovery = false) {
  clearNavigation();
  step(destination, recovery ? 'Archive' : 'Board items');
  alert.textContent = notice;
  const path = `/navigation/boards/${encodeURIComponent(id)}`;
  async function read() {
    const response = await fetchResponse(path);
    const groups = await response.json();
    const revision = Number(response.headers.get('boardcraft-board-revision'));
    const viewRevision = Number(response.headers.get('boardcraft-view-revision'));
    if (!Number.isSafeInteger(viewRevision) || viewRevision < 1) throw Error('Missing main table revision');
    return {id, revision, viewRevision, groups, rows: groups.flatMap(g => g.items), owner: response.headers.get('boardcraft-root-lifecycle') === 'owner', editable: response.headers.get('boardcraft-board-access') === 'edit'};
  }
  async function transition(item, action, draft) {
    const ack = await request(`${path}/items/${encodeURIComponent(item)}/${action}`, {method:'POST', body:JSON.stringify(draft)});
    if (ack.board_id !== id || ack.item_id !== item || ack.operation_id !== draft.operation_id) throw Error('Unexpected acknowledgment');
  }
  async function transitionGroup(group, action, revision) {
    const ack = await request(`${path}/groups/${encodeURIComponent(group)}/${action}`, {
      method:'POST', body:JSON.stringify({board_revision:revision})
    });
    if (ack.board_id !== id || ack.group_id !== group || ack.board_revision !== revision + 1) throw Error('Unexpected acknowledgment');
  }
  const readArchive = () => request(path+'/archive');
  const link = document.createElement('a');
  link.href = `/boards/${id}${recovery ? '' : '/archive'}`;
  link.textContent = recovery ? 'Close recovery' : 'Archive';
  link.onclick = e => { if (!e.ctrlKey && !e.shiftKey && !e.metaKey && !e.altKey) { e.preventDefault(); navigate(link.pathname); } };
  try {
    if (recovery) {
      const page = await readArchive();
      if (!current()) return;
      const mounted = visit = Symbol();
      mountArchive(items, page, {transition, transitionGroup, current: () => visit === mounted, expired: forgetSession, readBoard: read, refresh: () => run(loadRoute),
        more: (revision, offset) => request(`${path}/archive/groups/${revision}/${offset}`)});
      $('#recovery-nav').append(link); return;
    }
    const board = await read();
    if (!current()) return;
    const mounted = visit = Symbol();
    const menu = document.createElement('details'), label = document.createElement('summary');
    label.textContent = 'Archived & deleted content'; menu.append(label, link); $('#recovery-nav').append(menu);
    unmount = mountNames(board, items, $('#failures'), {
      current: () => visit === mounted,
      read,
      transition,
      transitionGroup, readArchive,
      saveFold: async (view_revision, group_ids, collapsed) => {
        const ack = await request(path+'/main-table-fold', {method:'POST', body:JSON.stringify({view_revision,group_ids,collapsed})});
        if (ack.board_id !== id || ack.view_revision !== view_revision + 1) throw Error('Unexpected acknowledgment');
      },
      reorderGroups: async (revision, ids) => {
        const ack = await request(path+'/group-order', {method:'POST', body:JSON.stringify({board_revision:revision, group_ids:ids})});
        if (ack.board_id !== id || ack.board_revision !== revision + 1) throw Error('Unexpected acknowledgment');
      },
      createGroup: draft => request(path+'/groups', {method:'POST', body:JSON.stringify(draft)}),
      editGroup: (id, draft) => request(`${path}/groups/${encodeURIComponent(id)}`, {method:'PUT', body:JSON.stringify(draft)}),
      moveSelection: draft => request(`${path}/items/move-selection`, {method:'POST', body:JSON.stringify(draft)}),
      archiveSelection: draft => request(`${path}/items/archive-selection`, {method:'POST', body:JSON.stringify(draft)}),
      reorder: (item, draft) => request(`${path}/items/${encodeURIComponent(item)}/reorder`, {
        method: 'POST', body: JSON.stringify(draft)
      }),
      move: (item, group_id, expected_revision) => request(`${path}/items/${encodeURIComponent(item)}/group`, {
        method: 'PUT', body: JSON.stringify({group_id, expected_revision})
      }),
      create: draft => request(`${path}/items`, {method: 'POST', body: JSON.stringify(draft)}),
      save: (item, edit) => request(`${path}/items/${encodeURIComponent(item)}/name`, {
        method: 'PUT', body: JSON.stringify(edit)
      }),
      expired: forgetSession,
      reopen: () => run(loadRoute),
      denied: (message = 'Name change denied.') => run(next => loadDestination(id, next, message)),
      reload: message => {
        clearNavigation();
        alert.textContent = message;
        retry.hidden = false; retry.onclick = () => run(loadRoute);
      }
    });
  } catch (error) {
    if (!current()) return;
    if (error === 401) return forgetSession();
    const message = error === 413 ? 'Board too large for this preview' :
      error === 403 || error === 404 ? 'Destination unavailable' : 'Could not load board';
    alert.textContent = [notice, message].filter(Boolean).join(' ');
    retry.onclick = () => run(loadRoute);
    retry.hidden = false;
  }
}

async function resolveSession(current) {
  clearNavigation();
  step(null, 'Checking session');
  try {
    const stored = await readBinding();
    if (!current()) return;
    binding = stored;
    if (!binding) return signInScreen();
    await request('/session');
  }
  catch (error) {
    if (!current()) return;
    if (error === 401) return forgetSession();
    step(resolve, 'Check connection');
    alert.textContent = 'Could not resolve session';
    return;
  }
  if (current()) await loadRoute(current);
}

function forgetSession() {
  const rejected = binding;
  return run(async current => {
    clearNavigation(); step(null, 'Checking session');
    try {
      const removed = await changeBinding(rejected, undefined);
      if (current()) return removed ? signInScreen() : resolveSession(current);
    } catch { if (current()) step(resolve, 'Check connection'); }
  });
}

identify.onsubmit = event => {
  event.preventDefault();
  if (active || !identifier.value) return;
  step(authenticate, 'Sign in'); password.focus();
};
$('#change').onclick = () => {
  authenticate.reset(); step(identify, 'Sign in'); identifier.focus();
};
authenticate.onsubmit = event => {
  event.preventDefault();
  if (active) return;
  run(async current => {
    try {
      const issued = await request('/session', {method: 'POST', body: JSON.stringify({
        identifier: identifier.value, password: password.value
       })});
      if (!current()) return;
      await changeBinding(undefined, issued);
    } catch (error) {
      if (!current()) return;
      alert.textContent = error === 401 ? 'Incorrect username or password' : 'Could not sign in';
      password.value = '';
      return;
    }
    if (!current()) return;
    authenticate.reset();
    await resolveSession(current);
  });
};
resolve.onclick = () => run(resolveSession);
refresh.onclick = () => run(loadRoute);
$('#back').onclick = () => navigate('/preview/');
$('#logout').onclick = () => run(async current => {
  try { await request('/session/logout', {method: 'POST'}); }
  catch (error) {
    if (current() && error === 401) return forgetSession();
    if (current()) alert.textContent = 'Could not log out. Choose Log out to retry.'; return;
  }
  if (current()) await forgetSession();
});
onChange(() => run(resolveSession));
window.addEventListener('pageshow', e => { if (e.persisted) run(resolveSession); });
run(resolveSession);
