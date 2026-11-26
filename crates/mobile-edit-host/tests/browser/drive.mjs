import {createInterface} from 'node:readline';
import {setTimeout as delay} from 'node:timers/promises';
import {launch} from './chromium.mjs';

const [profile, mode, port] = process.argv.slice(2);
const origin = `http://127.0.0.1:${port}`;
const browser = launch(profile, mode === 'foreground');
const send = browser.send;
const input = createInterface({input: process.stdin, terminal: false});
const check = (ok, checkpoint) => { if (!ok) throw Error(checkpoint); };
// Evaluate observations only. All interactions go through Chromium input dispatch.
async function observe(expression) {
  const value = await send('Runtime.evaluate', {expression, returnByValue: true});
  if (value.exceptionDetails) return undefined;
  return value.result.value;
}
async function wait(expression, label) {
  const end = Date.now() + 12000;
  do {
    if (await observe(expression)) return;
    await delay(40);
  } while (Date.now() < end);
  throw Error(label);
}
async function named(role, name) {
  const {nodes} = await send('Accessibility.getFullAXTree');
  check(nodes.some(n => !n.ignored && n.role?.value === role && n.name?.value === name), 'accessible control');
}
async function click(selector, name) {
  await named('button', name);
  const point = await observe(`(() => {
    const b = document.querySelector(${JSON.stringify(selector)}), r = b.getBoundingClientRect();
    const x = r.x + r.width/2, y = r.y + r.height/2;
    return !b.disabled && document.elementFromPoint(x,y) === b && {x,y};
  })()`);
  check(point, 'button hit target');
  for (const type of ['mousePressed', 'mouseReleased']) {
    await send('Input.dispatchMouseEvent', {type, ...point, button:'left', clickCount:1});
  }
}
async function key(key, code, vk, modifiers = 0, text) {
  await send('Input.dispatchKeyEvent', {type:'keyDown', key, code, windowsVirtualKeyCode:vk, modifiers, text});
  await send('Input.dispatchKeyEvent', {type:'keyUp', key, code, windowsVirtualKeyCode:vk, modifiers});
}
const enter = () => key('Enter','Enter',13,0,'\r');
const type = text => send('Input.insertText', {text});
function requests(expected) {
  check(JSON.stringify(browser.traffic.splice(0)) === JSON.stringify(expected.map(([m,p]) => [m,origin+p])), 'request order');
}
const login = ['POST','/session'], resolve = ['GET','/session'], list = ['GET','/navigation/boards'];
const focus = id => `document.activeElement.id === '${id}' && !document.activeElement.disabled`;
async function identifierScreen() {
  await wait(`${focus('identifier')} && document.activeElement.value === '' && !document.querySelector('li')
    && !document.querySelector('#boards').getBoundingClientRect().height
    && !document.querySelector('#destination').getBoundingClientRect().height
    && !document.querySelector('#authenticate').getBoundingClientRect().height`, 'clean identifier');
  await named('textbox','Username or email'); await named('button','Next');
}
async function passwordStep(identifier) {
  await type(identifier); await enter();
  await wait(`${focus('password')} && document.activeElement.type === 'password' && document.activeElement.required`, 'masked focus');
  await named('textbox','Password'); await named('button','Sign in');
  requests([]);
}
async function reject() {
  await type('incorrect'); await enter();
  await wait(`${focus('password')} && document.querySelector('[role=alert]').innerText === 'Incorrect username or password'
    && !document.querySelector('#boards').getBoundingClientRect().height`, 'retryable rejection');
  requests([login]);
}
async function visibleRows(expected, alert = '') {
  await wait(`document.querySelector('li') && !document.querySelector('#refresh').disabled`, 'board list');
  await named('list','Boards'); await named('button','Refresh'); await named('button','Log out');
  const rows = await observe(`Array.from(document.querySelectorAll('ul > li'), li => {
    const b = li.querySelector(':scope > button:only-child');
    const r = b.getBoundingClientRect();
    const hit = document.elementFromPoint(r.x + r.width/2, r.y + r.height/2);
    return [b.dataset.boardId, b.innerText, r.top, r.bottom, r.width, !b.disabled && hit === b && r.bottom <= innerHeight && r.top >= 0];
  })`);
  check(rows.length === expected.length && rows.every(([id,name,top,bottom,width,visible],i) =>
    id === expected[i][0] && name === expected[i][1] && width > 0 && visible && bottom > top
    && (!i || top >= rows[i-1][3])), 'ordered visible IDs and names');
  const {nodes} = await send('Accessibility.getFullAXTree');
  const names = nodes.filter(n => !n.ignored && n.role?.value === 'button').map(n => n.name.value);
  for (const [,name] of expected) {
    check(names.filter(label => label === name).length === expected.filter(r => r[1] === name).length, 'accessible board buttons');
  }
  check(await observe(`!document.querySelector('li a, #boards input, #boards textarea, [contenteditable=true]') && document.querySelector('#destination').hidden
    && document.querySelector('#empty').hidden && document.querySelector('[role=alert]').innerText === ${JSON.stringify(alert)}`), 'list presentation');
}
async function noRows(empty) {
  const label = empty ? 'Refresh' : 'Retry board list';
  await wait(`!document.querySelector('li') && !document.querySelector('#refresh').disabled
    && !document.querySelector('#boards').hidden && document.querySelector('#destination').hidden
    && document.querySelector('#refresh').innerText === '${label}'
    && (document.querySelector('#empty').getBoundingClientRect().height > 0) === ${empty}
    && document.querySelector('#empty').textContent === 'No boards'
    && document.querySelector('[role=alert]').innerText === '${empty ? '' : 'Could not load boards'}'`, label);
  await named('button', label);
}

// Fixture UUIDs, independent of the page's labels, dataset and requests.
const uuid = n => '00000000-0000-0000-0000-' + n.toString(16).padStart(12, '0');
const selectedGet = n => ['GET', `/navigation/boards/${uuid(n)}`];
async function destination(expected) {
  const denied = !expected;
  await wait(`${focus(denied ? 'retry-destination' : 'back')} && document.querySelector('#boards').hidden
    && !document.querySelector('[data-board-id]') && !document.querySelector('#destination').hidden
    && document.querySelector('[role=alert]').innerText === '${denied ? 'Destination unavailable' : ''}'`, 'destination result');
  await named('button', 'Back to boards');
  if (denied) await named('button', 'Retry destination');
  else await named('list', 'Board items');
  const rows = await observe(`Array.from(document.querySelectorAll('li'), li => {
    const r = li.getBoundingClientRect();
    return [li.dataset.itemId, li.innerText, r.top, r.bottom, document.elementFromPoint(r.x+r.width/2, r.y+r.height/2) === li];
  })`);
  check(rows.length === (expected?.length ?? 0) && rows.every(([id,title,top,bottom,hit], i) =>
    id === uuid(expected[i][0]) && title === expected[i][1] && hit && bottom > top && (!i || top >= rows[i-1][3])), 'ordered destination items');
  const visibleControls = await observe(`Array.from(document.querySelectorAll('input, textarea, button, a, [contenteditable=true]'))
    .filter(c => c.getClientRects().length).map(c => c.innerText)`);
  check(JSON.stringify(visibleControls) === JSON.stringify(denied ? ['Retry destination','Back to boards'] : ['Back to boards']), 'read only destination controls');
  check(JSON.stringify(browser.selectionStatuses.splice(0)) === JSON.stringify([denied ? 403 : 200]), 'selected response status');
}

async function perform({action, password, rows}) {
  switch (action) {
    case 'open':
      await browser.open(origin+'/preview/'); await identifierScreen(); requests([]); break;
    case 'reject':
      await enter(); await identifierScreen();
      await passwordStep('unknown'); await reject();
      await click('#change','Change');
      await wait(`${focus('identifier')} && document.activeElement.value === 'unknown' && document.querySelector('#password').value === '' && !document.querySelector('[role=alert]').innerText`, 'change identifier');
      await key('a','KeyA',65,2); await passwordStep('reader'); await reject(); break;
    case 'login':
      if (await observe(focus('identifier'))) await passwordStep('reader');
      await type(password); await enter(); await visibleRows(rows); requests([login,resolve,list]); break;
    case 'empty': case 'unavailable':
      await click('#refresh','Refresh'); await noRows(action === 'empty'); requests([list]); break;
    case 'retry':
      await noRows(false); requests([]);
      await click('#refresh','Retry board list'); await visibleRows(rows); requests([list]); break;
    case 'logout':
      await click('#logout','Log out'); await identifierScreen(); requests([['POST','/session/logout']]); break;
    case 'failed logout': {
      const alert = 'Could not log out. Choose Log out to retry.';
      await click('#logout','Log out');
      await wait(`document.querySelector('[role=alert]').innerText === ${JSON.stringify(alert)}`, 'retryable logout');
      await visibleRows(rows, alert); requests([['POST','/session/logout']]); break;
    }
    case 'expired':
      await click('#refresh','Refresh'); await identifierScreen(); requests([list]); break;
    case 'open last': case 'denied': case 'retry denied':
      if (action === 'retry denied') await click('#retry-destination','Retry destination');
      else await click('#boards li:last-child button','Review');
      await destination(action === 'open last' ? [[73,'Sketch'],[61,'Ink']] : null);
      requests([selectedGet(201)]); break;
    case 'open first review':
      await key('Tab','Tab',9);
      check(await observe(`document.activeElement.dataset.boardId === '${uuid(202)}'`), 'focused first Review');
      await enter(); await destination([[62,'Outline']]); requests([selectedGet(202)]); break;
    case 'back': case 'back empty':
      await click('#back','Back to boards');
      if (action === 'back') await visibleRows(rows); else await noRows(true);
      requests([list]); break;
    case 'refresh restored':
      await noRows(true); requests([]);
      await click('#refresh','Refresh'); await visibleRows(rows); requests([list]); break;
    case 'park': await browser.windowClosed; break;
    default: throw Error('unknown action');
  }
}
try {
  for await (const line of input) {
    const command = JSON.parse(line);
    await perform(command);
    process.stdout.write('ok\n');
  }
} catch (err) {
  // Only report our checkpoint, never dump CDP payloads, input, or page state.
  process.stderr.write(`Preview checkpoint failed: ${err.message.match(/^[a-zA-Z .:]{1,80}$/)?.[0] ?? 'driver error'}\n`);
  process.exitCode = 1;
} finally {
  input.close(); process.stdin.destroy(); await browser.close();
}
