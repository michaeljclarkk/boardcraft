import {createInterface} from 'node:readline';
import {setTimeout as delay} from 'node:timers/promises';
import {launch} from './chromium.mjs';
import {nameJourney} from './names.mjs';
import {createJourney} from './create.mjs';
import {moveJourney} from './moves.mjs';
import {reorderJourney} from './reorder.mjs';
import {selectionJourney} from './selection.mjs';
import {batchJourney} from './selection-move.mjs';
import {lifecycleJourney} from './lifecycle.mjs';
import {groupLifecycleJourney} from './group-lifecycle.mjs';
import {groupCreateJourney} from './group-create.mjs';
import {groupEditJourney} from './group-edit.mjs';
import {groupOrderJourney} from './group-order.mjs';

const [profile, mode, port] = process.argv.slice(2);
const primary = `http://127.0.0.1:${port}`;
let origin = primary;
const browser = launch(profile, mode === 'foreground');
const send = browser.send;
const input = createInterface({input: process.stdin, terminal: false});
const check = (ok, checkpoint) => { if (!ok) throw Error(checkpoint); };
// Evaluate observations only. Use Chromium input and native history commands for actions.
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
async function click(selector, name, role = 'button', count = 1, modifiers = 0, expectDisabled = false) {
  if (role) await named(role, name);
  const point = await observe(`(() => {
    const b = document.querySelector(${JSON.stringify(selector)}), r = b.getBoundingClientRect();
    const x = r.x + r.width/2, y = r.y + r.height/2;
    return Boolean(b.disabled) === ${expectDisabled} && document.elementFromPoint(x,y) === b && {x,y};
  })()`);
  check(point, 'control hit target');
  for (let i = 1; i <= count; i++) for (const type of ['mousePressed', 'mouseReleased']) {
    await send('Input.dispatchMouseEvent', {type, ...point, button:'left', clickCount:i, modifiers});
  }
}
async function key(key, code, vk, modifiers = 0, text) {
  await send('Input.dispatchKeyEvent', {type:'keyDown', key, code, windowsVirtualKeyCode:vk, modifiers, text});
  await send('Input.dispatchKeyEvent', {type:'keyUp', key, code, windowsVirtualKeyCode:vk, modifiers});
}
const enter = () => key('Enter','Enter',13,0,'\r');
const type = text => send('Input.insertText', {text});
function requests(expected) {
  check(JSON.stringify(browser.traffic.splice(0)) === JSON.stringify(expected.map(([m,p]) => [m,origin+'/preview/api'+p])), 'request order');
}
const login = ['POST','/session'], resolve = ['GET','/session'], list = ['GET','/navigation/boards'];
const focus = id => `document.activeElement.id === '${id}' && !document.activeElement.disabled`;
const root = '/preview/';
const boardPath = n => '/boards/' + uuid(n);
let entries, index;
const histories = new Map();
let tab = 'A';
async function choose(name, url) {
  histories.set(tab,{entries,index,origin}); tab = name;
  if (url) {
    entries = index = undefined; origin = new URL(url).origin;
    await browser.newTab(name,url);
    check(await observe('window.opener === null'), 'independent tab');
  } else {
    ({entries,index,origin} = histories.get(name)); browser.select(name);
  }
}
async function reload() {
  const old = await observe('performance.timeOrigin');
  await send('Page.reload'); await wait(`performance.timeOrigin !== ${old}`, 'new document');
}
function statuses(...expected) {
  check(JSON.stringify(browser.responses.splice(0).map(([p,s]) => [p,s])) ===
    JSON.stringify(expected.map(([req,s]) => ['/preview/api'+req[1],s])), 'response order and statuses');
}
async function historyAt(path, movement = 'stay') {
  const actual = await send('Page.getNavigationHistory');
  const urls = actual.entries.map(({id,url}) => ({id,url}));
  if (!entries) {
    check(urls.slice(0,-1).every(e => e.url === 'about:blank'), 'direct document entry');
    entries = urls; index = urls.length - 1;
  } else if (movement === 'push') {
    entries = [...entries.slice(0,index+1), urls[index+1]]; index++;
  } else if (movement === 'back') index--;
  else if (movement === 'forward') index++;
  check(index === actual.currentIndex && JSON.stringify(entries) === JSON.stringify(urls)
    && urls[index]?.url === origin+path && await observe('location.href') === origin+path, 'history entries and address');
}
async function traverse(delta) {
  await send('Page.navigateToHistoryEntry', {entryId: entries[index+delta].id});
}
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
    && !document.querySelector('li') && document.querySelector('#destination').hidden
    && !document.querySelector('#boards').getBoundingClientRect().height`, 'retryable rejection');
  requests([login]);
}
async function visibleRows(expected, alert = '') {
  await wait(`document.querySelector('li') && !document.querySelector('#refresh').disabled`, 'board list');
  await named('list','Boards'); await named('button','Refresh'); await named('button','Log out');
  const rows = await observe(`Array.from(document.querySelectorAll('ul > li'), li => {
    const b = li.querySelector(':scope > a[href]:only-child');
    const r = b.getBoundingClientRect();
    const hit = document.elementFromPoint(r.x + r.width/2, r.y + r.height/2);
    return [b.dataset.boardId, b.innerText, r.top, r.bottom, r.width, hit === b && r.bottom <= innerHeight && r.top >= 0, b.href];
  })`);
  check(rows.length === expected.length && rows.every(([id,name,top,bottom,width,visible,href],i) =>
    id === expected[i][0] && href === `${origin}/boards/${id}` && name === expected[i][1] && width > 0 && visible && bottom > top
    && (!i || top >= rows[i-1][3])), 'ordered visible IDs and names');
  const {nodes} = await send('Accessibility.getFullAXTree');
  const names = nodes.filter(n => !n.ignored && n.role?.value === 'link').map(n => n.name.value);
  for (const [,name] of expected) {
    check(names.filter(label => label === name).length === expected.filter(r => r[1] === name).length, 'accessible board links');
  }
  check(await observe(`!document.querySelector('li button, #boards input, #boards textarea, [contenteditable=true]') && document.querySelector('#destination').hidden
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
async function grouped(board, editable, saved = false, appended, moved = false, recovering = false) {
  const expected = board === 202 ? [['g-70','Work / 1 item',[[62,'Outline']]]] : [
    ['g-90','Work / 2 items',[[73,'Sketch'],[61,'Ink']]], ['g-10','Reserve / No Items',[]],
    ['g-20','Work / 1 item',[[59,saved ? 'Coral' : 'Ink']]]
  ];
  if (moved) {
    expected[2][2].unshift(expected[0][2].pop());
    expected[0][1] = 'Work / 1 item'; expected[2][1] = 'Work / 2 items';
  }
  const shape = expected.map(([id,heading,items]) => [id,heading,items.map(([n,title]) => [uuid(n),title,editable])]);
  if (appended) {
    expected[2][1] = shape[2][1] = 'Work / 2 items';
    shape[2][2].push([appended,'Ink',editable]);
  }
  await wait(`(() => {
    const visible = e => { const r=e.getBoundingClientRect(); return r.height>0 && e.contains(document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)); };
    const actual = [...document.querySelectorAll('#items > section')].map(s => {
      const h = s.querySelector('h2');
      return [s.dataset.groupId,visible(h) && h.innerText,[...s.querySelectorAll('li')].map(r =>
        [r.dataset.itemId,visible(r) && r.querySelector('.item-name')?.innerText,!!r.querySelector('button.item-name:enabled')])];
    });
    return JSON.stringify(actual)===${JSON.stringify(JSON.stringify(shape))} && !document.querySelector('#items li input:not([type=checkbox])') && (${recovering} || !document.querySelector('#failures button'));
  })()`, 'visible grouped board');
  for (const [,heading] of expected) { await named('region',heading); await named('heading',heading); }
}
async function destination(expected, fresh = true) {
  const denied = !expected;
  await wait(`${focus(denied ? 'retry-destination' : 'back')} && document.querySelector('#boards').hidden
    && !document.querySelector('[data-board-id]') && !document.querySelector('#destination').hidden
    && document.querySelector('[role=alert]').innerText === '${denied ? 'Destination unavailable' : ''}'`, 'destination result');
  await named('button', 'Back to boards');
  if (denied) await named('button', 'Retry destination');
  else await named('list', 'Board items');
  const rows = await observe(`Array.from(document.querySelectorAll('li'), li => {
    const r = li.getBoundingClientRect();
    return [li.dataset.itemId, li.querySelector('.item-name')?.innerText, r.top, r.bottom, document.elementFromPoint(r.x+r.width/2, r.y+r.height/2) === li];
  })`);
  check(rows.length === (expected?.length ?? 0) && rows.every(([id,title,top,bottom,hit], i) =>
    id === uuid(expected[i][0]) && title === expected[i][1] && hit && bottom > top && (!i || top >= rows[i-1][3])), 'ordered destination items');
  const visibleControls = await observe(`Array.from(document.querySelectorAll('input, textarea, button, a, [contenteditable=true]'))
    .filter(c => c.getClientRects().length && !c.matches('.row-menu [aria-expanded=false],.group-menu [aria-expanded=false],input[type=checkbox]')).map(c => c.innerText)`);
  check(JSON.stringify(visibleControls) === JSON.stringify(denied ? ['Retry destination','Back to boards'] : ['', '+ Add item', 'Back to boards']) &&
    await observe(`!document.querySelector('#items input:enabled:not([type=checkbox]), #items button:enabled:not([aria-expanded])')`), 'read only destination controls');
  check(JSON.stringify(browser.selectionStatuses.splice(0)) === JSON.stringify(fresh ? [denied ? 403 : 200] : []), 'selected response status');
}

const sketch = [[73,'Sketch'],[61,'Ink']], outline = [[62,'Outline']];
const logout = ['POST','/session/logout'];
async function retryOnly() {
  await wait(`document.querySelector('#resolve').getClientRects().length && !document.querySelector('#resolve').disabled
    && !document.querySelector('li, #items input, #failures button') &&
    [...document.querySelectorAll('button,input,a')].filter(e=>e.getClientRects().length).every(e=>e.id==='resolve')`, 'only retry connection');
  await named('button','Retry connection');
}
async function resumed(n, denied = false) {
  await destination(denied ? null : n === 201 ? sketch : outline);
  requests([resolve,selectedGet(n)]); statuses([resolve,200],[selectedGet(n),denied ? 403 : 200]); await historyAt(boardPath(n));
}
async function perform({action, password, rows, otherPort}) {
  if (action.startsWith('new group ')) return groupCreateJourney(action.slice(10), {browser,uuid,click,key,type,wait,check,requests,statuses,reload,selectedGet,resolve,observe,login,list,origin});
  if (action.startsWith('edit group ')) return groupEditJourney(action.slice(11), {browser,uuid,click,key,type,wait,check,requests,statuses,reload,selectedGet,resolve,observe,login,list,origin});
  if (action.startsWith('order groups ')) return groupOrderJourney(action.slice(13), {browser,uuid,click,key,wait,check,requests,statuses,reload,selectedGet,resolve,observe,list,origin});
  if (action.startsWith('group life ')) return groupLifecycleJourney(action.slice(11), {browser, uuid, click, key, wait, check, requests, statuses, reload, historyAt, traverse, boardPath, selectedGet, resolve, observe, login, list, origin});
  if (action.startsWith('life ')) return lifecycleJourney(action, {browser, uuid, click, key, wait, check, requests, statuses, reload, historyAt, boardPath, selectedGet, resolve, observe, named, traverse});
  if (action.startsWith('batch ')) return batchJourney(action, {browser, uuid, click, key, wait, check, requests, statuses, reload, historyAt, boardPath, selectedGet, resolve, observe, named});
  if (action.startsWith('select ')) return selectionJourney(action, {uuid, send, click, key, wait, requests, statuses, reload, historyAt, selectedGet, boardPath, resolve, observe, named});
  if (action.startsWith('reorder ')) return reorderJourney(action, {uuid, send, click, key, wait, requests, statuses, reload, historyAt, selectedGet, boardPath, resolve, observe});
  if (action.startsWith('move ')) return moveJourney(action, {browser, uuid, click, key, wait, check, grouped, requests, statuses, reload, historyAt, boardPath, selectedGet, resolve, observe});
  if (action.startsWith('create ')) return createJourney(action, {browser,click,enter,type,key,wait,check,grouped,
    requests,statuses,reload,historyAt,boardPath,selectedGet,resolve,observe});
  switch (action) {
    case 'groups member': case 'groups viewer':
      statuses([login,200],[resolve,200],[list,200]);
      await send('Emulation.setDeviceMetricsOverride',{width:1000,height:900,deviceScaleFactor:1,mobile:false});
      await click('#boards li:last-child a','Review','link'); await grouped(201,action==='groups member');
      requests([selectedGet(201)]); statuses([selectedGet(201),200]); await historyAt(boardPath(201),'push'); break;
    case 'groups save': {
      const row = `[data-group-id="g-20"] [data-item-id="${uuid(59)}"]`;
      await click(row+' button','Ink'); await key('F2','F2',113);
      await wait(`document.activeElement.matches('${row} input') && document.activeElement.value==='Ink'`, 'group editor identity');
      await key('a','KeyA',65,2); await type(' Coral '); await enter(); await grouped(201,true,true);
      const put = ['PUT',selectedGet(201)[1]+`/items/${uuid(59)}/name`];
      requests([put]); statuses([put,200]); await historyAt(boardPath(201)); break;
    }
    case 'groups reload':
      await reload(); await grouped(201,true,true); requests([resolve,selectedGet(201)]);
      statuses([resolve,200],[selectedGet(201),200]); await historyAt(boardPath(201)); break;
    case 'groups sibling':
      statuses([list,200]); await click(`[data-board-id="${uuid(202)}"]`,'Review','link');
      await grouped(202,true); requests([selectedGet(202)]); statuses([selectedGet(202),200]); await historyAt(boardPath(202),'push'); break;
    case 'groups denied':
      check(JSON.stringify(browser.selectionStatuses.splice(0))==='[200]', 'initial grouped read');
      await reload(); await resumed(201,true);
      check(await observe(`!document.querySelector('#items > *, #failures > *')`), 'denied groups removed'); break;
    case 'groups retry':
      await click('#retry-destination','Retry destination'); await grouped(201,false); requests([selectedGet(201)]);
      statuses([selectedGet(201),200]); await historyAt(boardPath(201)); break;
    case 'waiting sibling':
      await choose('B',primary+boardPath(202)); await identifierScreen(); await historyAt(boardPath(202)); requests([]); statuses();
      await choose('A'); break;
    case 'sibling saw login':
      await choose('B'); await resumed(202); await choose('A'); break;
    case 'sibling saw rejection':
      await choose('B'); await identifierScreen(); await historyAt(boardPath(202)); requests([]); statuses();
      await choose('A'); break;
    case 'reload demoted':
      // The preceding editable visit is checked by the packet 29 action.
      check(JSON.stringify(browser.selectionStatuses.splice(0))==='[200]', 'initial selected status');
      await reload(); await resumed(201); break;
    case 'open sibling tab':
      await choose('B',primary+boardPath(202)); await resumed(202);
      await choose('A'); await destination(sketch,false); await historyAt(boardPath(201)); requests([]); statuses();
      await choose('B'); break;
    case 'sibling denied': await reload(); await resumed(202,true); break;
    case 'retry sibling':
      await click('#retry-destination','Retry destination'); await destination(outline);
      requests([selectedGet(202)]); statuses([selectedGet(202),200]); await historyAt(boardPath(202)); break;
    case 'peer entry': {
      const peerOrigin = `http://127.0.0.1:${otherPort}`;
      await choose('peer',peerOrigin+'/preview/api/session');
      check(await browser.isolatedProbe(), 'second port receives only inert cookie');
      await send('Page.navigate',{url:peerOrigin+root}); await identifierScreen(); requests([]); break;
    }
    case 'peer login':
      await passwordStep('reader'); await type(password); await enter(); await visibleRows(rows);
      requests([login,resolve,list]); statuses([login,200],[resolve,200],[list,200]);
      await choose('A'); break;
    case 'failed logout status':
      statuses([list,200],[logout,503]); await historyAt(root); await choose('B');
      await destination(outline,false); requests([]); statuses(); break;
    case 'sibling retained': await reload(); await resumed(202); await choose('A'); break;
    case 'both signed out':
      statuses([logout,200]); await historyAt(root); await choose('B');
      await identifierScreen(); await historyAt(boardPath(202)); requests([]); statuses(); break;
    case 'reload signed out': await reload(); await identifierScreen(); await historyAt(boardPath(202)); requests([]); statuses(); break;
    case 'peer survives':
      await choose('peer'); await reload(); await visibleRows(rows); requests([resolve,list]); statuses([resolve,200],[list,200]); break;
    case 'unavailable resume':
      statuses([login,200],[resolve,200],[list,200],[selectedGet(201),200]);
      await reload(); await retryOnly(); requests([resolve]); statuses([resolve,503]); await historyAt(boardPath(201)); break;
    case 'retry resume':
      await retryOnly(); requests([]); statuses(); await click('#resolve','Retry connection'); await resumed(201); break;
    case 'rejected resume':
      await reload(); await identifierScreen(); requests([resolve]); statuses([resolve,401]); await historyAt(boardPath(201)); break;
    case 'forgotten resume':
      await reload(); await identifierScreen(); requests([]); statuses(); await historyAt(boardPath(201)); break;
    case 'open': case 'open member': case 'open direct': {
      const path = action === 'open direct' ? boardPath(202) : root;
      await browser.open(origin+path, action === 'open member'); await identifierScreen(); requests([]); await historyAt(path); break;
    }
    case 'direct identifier':
      await passwordStep('reader'); await historyAt(boardPath(202)); break;
    case 'direct rejection':
      await reject(); await historyAt(boardPath(202)); break;
    case 'direct login':
      await type(password); await enter(); await destination([[62,'Outline']]);
      requests([login,resolve,selectedGet(202)]); await historyAt(boardPath(202)); break;
    case 'reject':
      await enter(); await identifierScreen();
      await passwordStep('unknown'); await reject();
      await click('#change','Change');
      await wait(`${focus('identifier')} && document.activeElement.value === 'unknown' && document.querySelector('#password').value === '' && !document.querySelector('[role=alert]').innerText`, 'change identifier');
      await key('a','KeyA',65,2); await passwordStep('reader'); await reject(); break;
    case 'login':
      if (await observe(focus('identifier'))) await passwordStep('reader');
      await type(password); await enter(); await visibleRows(rows); requests([login,resolve,list]); await historyAt(root); break;
    case 'empty': case 'unavailable':
      await click('#refresh','Refresh'); await noRows(action === 'empty'); requests([list]); await historyAt(root); break;
    case 'retry':
      await noRows(false); requests([]);
      await click('#refresh','Retry board list'); await visibleRows(rows); requests([list]); await historyAt(root); break;
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
    case 'open last': case 'denied': case 'retry denied': case 'retry restored':
    case 'forward': case 'forward denied': {
      let movement = 'push';
      if (action.startsWith('retry')) { await click('#retry-destination','Retry destination'); movement = 'stay'; }
      else if (action.startsWith('forward')) { await traverse(1); movement = 'forward'; }
      else await click('#boards li:last-child a','Review','link');
      await destination(action.endsWith('denied') ? null : [[73,'Sketch'],[61,'Ink']]);
      requests([selectedGet(201)]); await historyAt(boardPath(201), movement); break;
    }
    case 'open first review':
      await key('Tab','Tab',9);
      check(await observe(`document.activeElement.dataset.boardId === '${uuid(202)}'`), 'focused first Review');
      await enter(); await destination([[62,'Outline']]); requests([selectedGet(202)]); await historyAt(boardPath(202),'push'); break;
    case 'back': case 'back empty': case 'history back':
      if (action === 'history back') await traverse(-1); else await click('#back','Back to boards');
      if (action === 'back empty') await noRows(true); else await visibleRows(rows);
      requests([list]); await historyAt(root, action === 'history back' ? 'back' : 'push'); break;
    case 'forward then back pending':
      await traverse(1);
      await wait(`!document.querySelector('#destination').hidden && document.querySelector('#back').disabled
        && !document.querySelector('li')`, 'pending selected read');
      await historyAt(boardPath(201),'forward'); await named('heading','Board items'); requests([selectedGet(201)]);
      await traverse(-1);
      await wait(`!document.querySelector('#boards').hidden && document.querySelector('#refresh').disabled
        && !document.querySelector('li')`, 'pending list read');
      await historyAt(root,'back'); await named('heading','Available boards'); requests([list]); break;
    case 'settled list':
      await visibleRows(rows); await historyAt(root); requests([]);
      check(JSON.stringify(browser.selectionStatuses.splice(0)) === '[200]', 'superseded selected response'); break;
    case 'refresh restored':
      await noRows(true); requests([]);
      await click('#refresh','Refresh'); await visibleRows(rows); requests([list]); await historyAt(root); break;
    case 'park': await browser.windowClosed; break;
    default: await nameJourney(action, {uuid, wait, observe, named, click, key, enter, type, requests, browser, check, historyAt});
  }
}
try {
  for await (const line of input) {
    const command = JSON.parse(line);
    const item = await perform(command);
    process.stdout.write(item ? `ok ${item}\n` : 'ok\n');
  }
} catch (err) {
  // Only report our checkpoint, never dump CDP payloads, input, or page state.
  process.stderr.write(`Preview checkpoint failed: ${err.message.match(/^[a-zA-Z .:]{1,80}$/)?.[0] ?? 'driver error'}\n`);
  process.exitCode = 1;
} finally {
  input.close(); process.stdin.destroy(); await browser.close();
}
