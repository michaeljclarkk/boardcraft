// Real input commands; evaluate only for focus, geometry and mounted text.
export async function nameJourney(action, {uuid, wait, observe, named, click, key, enter, type, requests, browser, check, historyAt}) {
  const path = `/navigation/boards/${uuid(201)}`, put = `${path}/items/${uuid(61)}/name`;
  const row = id => `[data-item-id="${uuid(id)}"]`, second = row(61)+' button';
  const response = (p, status, hint = null) => ['/preview/api'+p, status, hint];
  const responses = (...expected) => check(JSON.stringify(browser.responses.splice(0)) === JSON.stringify(expected), 'response order and access');
  const f2 = () => key('F2','F2',113), escape = () => key('Escape','Escape',27);
  async function displayed(title = 'Ink', failed = false, editable = true) {
    await wait(`(() => {
      const rows = [...document.querySelectorAll('#items li')];
      const failures = document.querySelector('#failures');
      return rows.length === 2 && rows.every((li,i) => {
        const r = li.getBoundingClientRect();
        return li.dataset.itemId === ['${uuid(73)}','${uuid(61)}'][i] &&
          li.innerText === ['Sketch',${JSON.stringify(title)}][i] &&
          !!li.querySelector('button') === ${editable} && li.contains(document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)) &&
          r.height > 0 && (!i || r.top >= rows[0].getBoundingClientRect().bottom);
      }) && !document.querySelector('#items li input') && failures.innerText.includes('Could not rename Ink.') === ${failed} &&
        failures.querySelectorAll('button').length === ${failed ? 2 : 0};
    })()`, 'visible name cells and recovery');
    if (failed) { await named('button','Retry change'); await named('button','Dismiss'); }
  }
  async function editor(text) {
    await named('textbox','Edit item name');
    await wait(`document.activeElement.matches('${row(61)} input') && document.activeElement.value === ${JSON.stringify(text)}`, 'focused name draft');
  }
  async function doubleOpen() { await click(second,'Ink','button',2); await editor('Ink'); }
  async function replace(text) { await key('a','KeyA',65,2); await type(text); await editor(text); }
  async function payload(title) {
    check(browser.renameIds.length === 1, 'one synthetic rename');
    const requestId = browser.renameIds.shift();
    const edit = JSON.parse((await browser.send('Network.getRequestPostData',{requestId})).postData);
    check(edit.title === title && edit.revision === 1 && Object.keys(edit).length === 2, 'entry revision and raw spelling');
    return requestId;
  }
  switch (action) {
    case 'open editable': case 'reopen saved': case 'reopen readonly': {
      responses(...(action === 'open editable' ? [response('/session',200),response('/session',200)] : []), response('/navigation/boards',200));
      await click(`[data-board-id="${uuid(201)}"]`, 'Review', 'link');
      const editable = action !== 'reopen readonly';
      await displayed(action === 'reopen saved' ? 'Coral' : 'Ink', false, editable);
      await historyAt(`/boards/${uuid(201)}`, 'push');
      requests([['GET',path]]); responses(response(path,200,editable ? 'edit' : 'read')); break;
    }
    case 'cancel name': case 'unchanged name': {
      await click(second,'Ink');
      check(await observe(`document.activeElement.matches('${second}') && !document.querySelector('#items li input')`), 'single click focuses name');
      if (action === 'cancel name') { await f2(); await editor('Ink'); await replace('Draft'); await escape(); }
      else { await enter(); await editor('Ink'); await enter(); }
      await displayed(); requests([]); responses(); break;
    }
    case 'blank blur':
      await doubleOpen(); await replace('   '); await click(row(73)+' button','Sketch');
      await displayed('Ink',true);
      check(await observe(`document.activeElement.matches('${row(73)} button')`), 'blur preserved sibling focus');
      requests([['PUT',put]]); responses(response(put,422)); await payload('   '); break;
    case 'cancel rejected':
      await doubleOpen(); await replace('Other'); await escape(); await displayed('Ink',true); requests([]); responses(); break;
    case 'retry name':
      await click('#failures button','Retry change'); await displayed('Ink',true);
      // The old alert is still visible during the detail read.
      await untilResponses(browser, 2);
      await displayed('Ink',true); requests([['GET',path],['PUT',put]]);
      responses(response(path,200,'edit'),response(put,422)); await payload('   '); break;
    case 'dismiss name':
      await click('#failures button:last-child','Dismiss'); await displayed(); requests([]); responses(); break;
    case 'draft coral':
      await doubleOpen(); await replace(' Coral '); requests([]); responses(); break;
    case 'pending name':
      await enter(); await displayed(' Coral '); requests([['PUT',put]]); responses();
      pending = await payload(' Coral '); break;
    case 'saved name': {
      await displayed('Coral'); requests([]); responses(response(put,200));
      const saved = JSON.parse((await browser.send('Network.getResponseBody',{requestId: pending})).body);
      check(saved.id === uuid(61) && saved.title === 'Coral' && saved.revision === 2, 'accepted rename row'); break;
    }
    case 'denied name':
      await enter(); await displayed('Ink',false,false);
      check(await observe(`document.querySelector('main > [role=alert]').innerText === 'Name change denied.'`), 'denial visible');
      requests([['PUT',put],['GET',path]]); responses(response(put,403),response(path,200,'read')); await payload(' Coral '); break;
    case 'readonly attempts':
      await click(row(61),'Ink',null,2); await f2(); await enter(); await displayed('Ink',false,false); requests([]); responses(); break;
    default: throw Error('unknown name action');
  }
}

import {setTimeout as delay} from 'node:timers/promises';
let pending;
async function untilResponses(browser, count) {
  const end = Date.now()+12000;
  while (browser.responses.length < count && Date.now() < end) await delay(20);
}
