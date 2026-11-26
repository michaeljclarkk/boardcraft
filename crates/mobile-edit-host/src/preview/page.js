const $ = selector => document.querySelector(selector);
const identify = $('#identify'), authenticate = $('#authenticate'), boards = $('#boards');
const identifier = $('#identifier'), password = $('#password'), alert = $('[role=alert]');
const refresh = $('#refresh'), resolve = $('#resolve'), list = $('ul');
let token, busy = false;

function step(panel, title) {
  for (const view of [identify, authenticate, boards, resolve]) view.hidden = view !== panel;
  $('#heading').textContent = title;
  alert.textContent = '';
}

function signInScreen() {
  token = undefined;
  identify.reset(); authenticate.reset();
  list.replaceChildren(); $('#empty').hidden = true;
  step(identify, 'Sign in');
}

// Keep the submit guard across login -> resolve -> list, including keyboard submits.
async function run(operation) {
  if (busy) return;
  busy = true;
  const controls = document.querySelectorAll('input, button');
  for (const control of controls) control.disabled = true;
  alert.textContent = '';
  try { await operation(); }
  finally {
    busy = false;
    for (const control of controls) control.disabled = false;
    if (!identify.hidden) identifier.focus();
    else if (!authenticate.hidden) password.focus();
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

async function loadBoards() {
  list.replaceChildren(); $('#empty').hidden = true;
  step(boards, 'Available boards');
  try {
    const rows = await request('/navigation/boards');
    const items = rows.map(({id, name}) => {
      const item = document.createElement('li');
      item.dataset.boardId = id; item.textContent = name;
      return item;
    });
    list.replaceChildren(...items);
    $('#empty').hidden = items.length !== 0;
    refresh.textContent = 'Refresh';
  } catch (error) {
    if (error === 401) return signInScreen();
    refresh.textContent = 'Retry board list';
    alert.textContent = 'Could not load boards';
  }
}

async function resolveSession() {
  try { await request('/session'); }
  catch (error) {
    if (error === 401) return signInScreen();
    step(resolve, 'Check connection');
    alert.textContent = 'Could not resolve session';
    return;
  }
  await loadBoards();
}

identify.onsubmit = event => {
  event.preventDefault();
  if (busy || !identifier.value) return;
  step(authenticate, 'Sign in'); password.focus();
};
$('#change').onclick = () => {
  authenticate.reset(); step(identify, 'Sign in'); identifier.focus();
};
authenticate.onsubmit = event => {
  event.preventDefault();
  run(async () => {
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
    await resolveSession();
  });
};
resolve.onclick = () => run(resolveSession);
refresh.onclick = () => run(loadBoards);
$('#logout').onclick = () => run(async () => {
  try { await request('/session/logout', {method: 'POST'}); }
  catch { alert.textContent = 'Could not log out. Choose Log out to retry.'; return; }
  signInScreen();
});
