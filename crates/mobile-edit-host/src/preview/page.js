const $ = selector => document.querySelector(selector);
const identify = $('#identify'), authenticate = $('#authenticate'), boards = $('#boards');
const identifier = $('#identifier'), password = $('#password'), alert = $('[role=alert]');
const refresh = $('#refresh'), resolve = $('#resolve'), list = $('#boards ul');
const destination = $('#destination'), items = $('#items'), retry = $('#retry-destination');
let token, active;

function step(panel, title) {
  for (const view of [identify, authenticate, boards, destination, resolve]) view.hidden = view !== panel;
  $('#heading').textContent = title;
  alert.textContent = '';
}

function clearNavigation() {
  list.replaceChildren(); $('#empty').hidden = true;
  items.replaceChildren(); retry.hidden = true; retry.onclick = null;
}

function signInScreen() {
  token = undefined;
  identify.reset(); authenticate.reset();
  clearNavigation();
  step(identify, 'Sign in');
}

// History traversal can start a new read while an older one is still in flight.
async function run(operation) {
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

async function request(path, options = {}) {
  const response = await fetch(path, {
    mode: 'same-origin', credentials: 'omit', redirect: 'error', cache: 'no-store',
    signal: AbortSignal.timeout(15000),
    headers: token ? { Authorization: `Bearer ${token}` } : { 'Content-Type': 'application/json' },
    ...options
  });
  if (!response.ok) throw response.status;
  return response.json();
}

function navigate(path) {
  history.pushState(null, '', path);
  run(loadRoute);
}

function loadRoute(current) {
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
    if (error === 401) return signInScreen();
    refresh.textContent = 'Retry board list';
    alert.textContent = 'Could not load boards';
  }
}

async function loadDestination(id, current) {
  clearNavigation();
  step(destination, 'Board items');
  try {
    const rows = await request(`/navigation/boards/${encodeURIComponent(id)}`);
    if (!current()) return;
    items.replaceChildren(...rows.map(({id, title}) => {
      const item = document.createElement('li');
      item.dataset.itemId = id; item.textContent = title;
      return item;
    }));
  } catch (error) {
    if (!current()) return;
    if (error === 401) return signInScreen();
    alert.textContent = error === 403 || error === 404 ? 'Destination unavailable' : 'Could not load board';
    retry.onclick = () => run(loadRoute);
    retry.hidden = false;
  }
}

async function resolveSession(current) {
  try { await request('/session'); }
  catch (error) {
    if (error === 401) return signInScreen();
    step(resolve, 'Check connection');
    alert.textContent = 'Could not resolve session';
    return;
  }
  await loadRoute(current);
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
      token = await request('/session', {method: 'POST', body: JSON.stringify({
        identifier: identifier.value, password: password.value
      })});
    } catch (error) {
      alert.textContent = error === 401 ? 'Incorrect username or password' : 'Could not sign in';
      password.value = '';
      return;
    }
    authenticate.reset();
    await resolveSession(current);
  });
};
resolve.onclick = () => run(resolveSession);
refresh.onclick = () => run(loadRoute);
$('#back').onclick = () => navigate('/preview/');
$('#logout').onclick = () => run(async current => {
  const revoked = token;
  try { await request('/session/logout', {method: 'POST'}); }
  catch { if (current()) alert.textContent = 'Could not log out. Choose Log out to retry.'; return; }
  if (token === revoked) await run(signInScreen);
});
