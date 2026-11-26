let proposed, pending;

export async function createJourney(action, {browser,click,enter,type,key,wait,check,grouped,
  requests,statuses,reload,historyAt,boardPath,selectedGet,resolve,observe}) {
  const get = selectedGet(201), post = ['POST', get[1] + '/items'];
  const form = '[data-group-id="g-20"] .add-item', input = form+' input';
  const field = `document.querySelector('${input}')`;
  const over = '😀'.repeat(128);
  async function payload(raw) {
    check(browser.createIds.length === 1,'one synthetic create');
    const requestId = browser.createIds.shift();
    const draft = JSON.parse((await browser.send('Network.getRequestPostData',{requestId})).postData);
    check(Object.keys(draft).sort().join() === 'group_id,id,title' && draft.group_id === 'g-20' && draft.title === raw &&
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(draft.id), 'group create payload');
    return [draft.id,requestId];
  }
  async function readonly() {
    await grouped(201,false);
    check(await observe(`document.querySelectorAll('.add-item input:disabled').length === 3 &&
      document.querySelectorAll('.add-item button:disabled').length === 3 && !document.querySelector('.add-item input:enabled')`), 'readonly add controls');
  }
  switch (action) {
    case 'create blank and cancel':
      await click(input,'Add item to Work','textbox'); await type('   '); await enter();
      await type('Discard'); await key('Escape','Escape',27);
      await wait(`${field}.value === ''`, 'cancelled add draft'); requests([]); statuses(); break;
    case 'create rejected':
      await type(over); await enter();
      await wait(`${field}.value === ${JSON.stringify(over)} && !${field}.disabled &&
        document.querySelector('${form} button').innerText === 'Retry' && document.querySelector('${form} [role=status]').innerText`, 'retained rejection');
      await grouped(201,true); requests([post]); statuses([post,422]);
      [proposed] = await payload(over); return proposed;
    case 'create corrected':
      await key('a','KeyA',65,2); await type(' Ink ');
      await wait(`${field}.value === ' Ink '`, 'corrected add draft'); requests([]); statuses(); break;
    case 'create pending': {
      await enter(); await wait(`${field}.disabled && document.querySelector('${form} [role=status]').innerText === 'Saving…'`, 'pending add draft');
      await grouped(201,true); requests([post]); statuses();
      const [id,req] = await payload(' Ink '); check(id === proposed,'same draft identity'); pending = req; break;
    }
    case 'create saved': {
      await grouped(201,true,false,proposed); requests([get]); statuses([post,200],[get,200]);
      const row = JSON.parse((await browser.send('Network.getResponseBody',{requestId:pending})).body);
      check(row.id === proposed && row.group_id === 'g-20' && row.revision === 1 && row.title === 'Ink','committed create response');
      check(await observe(`!${field}.disabled && ${field}.value === ''`),'new empty draft'); break;
    }
    case 'create reload': case 'create viewer reload':
      await reload();
      if (action === 'create reload') await grouped(201,true,false,proposed); else await readonly();
      requests([resolve,get]); statuses([resolve,200],[get,200]); break;
    case 'create member draft':
      await click(input,'Add item to Work','textbox'); await type(' Ink ');
      await wait(`${field}.value === ' Ink '`, 'member draft'); requests([]); statuses(); break;
    case 'create denied':
      await enter(); await readonly();
      check(await observe(`document.querySelector('main > [role=alert]').innerText === 'Item creation denied.'`), 'denial notice');
      requests([post,get]); statuses([post,403],[get,200]);
      return (await payload(' Ink '))[0];
    default: throw Error('unknown create checkpoint');
  }
  await historyAt(boardPath(201));
}
