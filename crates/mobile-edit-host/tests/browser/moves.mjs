let pending;
export async function moveJourney(action, {browser, uuid, click, key, wait, check, grouped, requests, statuses, reload, historyAt, boardPath, selectedGet, resolve, observe}) {
  const menu = `[data-item-id="${uuid(61)}"] .row-menu`, get = selectedGet(201);
  const put = ['PUT', get[1] + `/items/${uuid(61)}/group`];
  const confirm = () => click('dialog button', 'Confirm');
  const failure = '#failures > :last-child';
  async function payload(revision = 1) {
    check(browser.moveIds.length === 1,'one move request');
    const requestId = browser.moveIds.shift();
    const body = JSON.parse((await browser.send('Network.getRequestPostData',{requestId})).postData);
    check(Object.keys(body).sort().join() === 'expected_revision,group_id' && body.group_id === 'g-20' && body.expected_revision === revision,'move draft');
    return requestId;
  }
  async function accepted(requestId) {
    const row = JSON.parse((await browser.send('Network.getResponseBody',{requestId})).body);
    check(row.id === uuid(61) && row.title === 'Ink' && row.revision === 2 && row.group_id === 'g-20','committed move response');
  }
  async function choose() {
    await click(menu+' button', 'More options for Ink');
    await click(menu+' button:last-child','Move to');
    await wait(`document.activeElement.matches('dialog[open] select')`, 'move dialog focus');
    const choices = await observe(`Array.from(document.activeElement.options,o=>[o.value,o.text])`);
    check(JSON.stringify(choices) === JSON.stringify([['g-90','Work'],['g-10','Reserve'],['g-20','Work']]) &&
      await observe(`document.activeElement.value === 'g-90' && document.querySelector('dialog select:disabled').value === '${uuid(201)}'`),'ordered group choices');
    await key('End','End',35);
    await wait(`document.activeElement.value === 'g-20'`, 'last Work selected');
    requests([get]); statuses([get,200]);
  }
  switch (action) {
    case 'move cancel':
      await choose(); await click('dialog button:last-of-type','Cancel');
      await grouped(201,true); requests([]); statuses(); break;
    case 'move choose': await choose(); break;
    case 'move pending':
      await confirm();
      await wait(`document.querySelector('dialog button:disabled') && document.querySelector('dialog [role=status]').innerText === 'Saving…' && [...document.querySelectorAll('#items h2')].slice(0,3).map(h=>h.innerText).join('|') === 'Work / 2 items|Reserve / No Items|Work / 1 item'`, 'old counts during save');
      requests([put]); statuses(); pending = await payload(); break;
    case 'move saved': await grouped(201,true,false,null,true); requests([get]); statuses([put,200],[get,200]); await accepted(pending); break;
    case 'move denied':
      await confirm(); await grouped(201,false); check(await observe(`document.querySelector('main > [role=alert]').innerText === 'Item move denied.'`),'move denial');
      requests([put,get]); statuses([put,403],[get,200]); await payload(); break;
    case 'move failed':
      await confirm(); await wait(`!document.querySelector('dialog') && document.querySelector('${failure} [role=alert]')?.innerText === 'Could not confirm move.'`,'move failure');
      await grouped(201,true,false,null,false,true); requests([put]); statuses([put,500]); await payload(); break;
    case 'move review':
      await click(failure+' button','Review move');
      await wait(`Array.from(document.querySelectorAll('${failure} [data-group-id]'),e=>e.dataset.groupId+':'+e.innerText).join('|') === 'g-90:Current group: Work (g-90)|g-20:Requested group: Work (g-20)'`, 'review identities');
      requests([get]); statuses([get,200]); break;
    case 'move changed before open':
      await choose(); await confirm();
      await wait(`!document.querySelector('dialog') && document.querySelector('${failure} [role=alert]')?.innerText === 'Group changed. Review move.'`, 'changed source requires review');
      await grouped(201,true,false,null,false,true); requests([]); statuses(); break;
    case 'move apply changed review':
      await click(failure+' button:nth-last-child(2)','Apply reviewed move');
      await wait(`document.querySelector('${failure} [role=alert]')?.innerText === 'Group changed. Review move.' && !document.querySelector('${failure} [data-group-id]')`,'server rejected stale review');
      requests([put]); statuses([put,409]); await payload(3); break;
    case 'move dismiss':
      await click(failure+' button:last-child','Dismiss'); await grouped(201,true); requests([]); statuses(); break;
    case 'move apply':
      await click(failure+' button:nth-last-child(2)','Apply reviewed move'); await grouped(201,true,false,null,true);
      requests([put,get]); statuses([put,200],[get,200]); await accepted(await payload()); break;
    case 'move reload': case 'move viewer reload': {
      const viewer = action === 'move viewer reload';
      await reload(); await grouped(201,!viewer,false,null,!viewer);
      requests([resolve,get]); statuses([resolve,200],[get,200]);
      if (viewer) {
        await click(menu+' button','More options for Ink');
        check(await observe(`document.querySelector('${menu} button:last-child').matches(':disabled:not([hidden])')`),'disabled Move to'); requests([]); statuses();
      }
      break;
    }
    default: throw Error('unknown move checkpoint');
  }
  await historyAt(boardPath(201));
}
