let operation;
export async function lifecycleJourney(action, {browser, uuid, click, key, wait, check, requests, statuses, reload, historyAt, boardPath, selectedGet, resolve, observe, named, traverse}) {
  const get = selectedGet(201), inventory = ['GET', get[1]+'/archive'];
  const post = kind => ['POST', get[1]+`/items/${uuid(67)}/${kind}`];
  const row = `[data-item-id="${uuid(67)}"]`, menu = row+' .row-menu', dlg = 'dialog[open]';
  const path = boardPath(201), recovery = path+'/archive';
  const exchange = (...pairs) => { requests(pairs.map(([req])=>req)); statuses(...pairs); };
  async function command(b,i,same=false) {
    check(browser.lifecycleIds.length===1,'one lifecycle command');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.lifecycleIds.shift()});
    const d=JSON.parse(postData);
    check(Object.keys(d).sort().join()==='board_revision,item_revision,operation_id' && d.board_revision===b && d.item_revision===i && /^[0-9a-f-]{36}$/.test(d.operation_id) && ((operation===d.operation_id)===same),'command baseline and identity'); operation=d.operation_id;
  }
  async function grid(archived){
    const expected = archived ? [[73,61,77],[],[59]] : [[73,61,67,77],[],[59]];
    const shape=expected.map((ids,i)=>[['g-90','g-10','g-20'][i],['Work','Reserve','Work'][i]+' / '+(!ids.length?'No Items':ids.length+(ids.length===1?' item':' items')),ids.map(uuid)]);
    await wait(`JSON.stringify([...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,s.querySelector('h2').innerText,[...s.querySelectorAll('li')].map(r=>r.dataset.itemId)]))===${JSON.stringify(JSON.stringify(shape))}`, 'lifecycle grid');
  }
  const more=()=>click(menu+' button','More options for Ink');
  async function open(){await more();await click(menu+' .archive-item','Archive');await named('dialog','Archive selected items?'); await wait(`document.querySelector('dialog p').innerText.includes('1 item: Ink')`, 'archive summary');}
  const confirm=()=>click(dlg+' button','Confirm');
  const restore=()=>click(row+' button','Restore item');
  async function listing(archived,owner=true){
    const shape=[[uuid(57),'Stored · Retired',true],...(archived?[[uuid(67),'Ink · Work',!owner]]:[])];
    await wait(`document.querySelector('#items p')?.innerText==='${shape.length} archived item${archived?'s':''}' && JSON.stringify([...document.querySelectorAll('#items li[data-item-id]')].map(r=>[r.dataset.itemId,r.firstChild.innerText,r.querySelector('button').disabled]))===${JSON.stringify(JSON.stringify(shape))} && document.querySelector('#items li').innerText.includes('Restore its group first.')`, 'board archive');
    if (!owner) await wait(`document.querySelector('#items').innerText.includes('Owner access required.')`, 'read-only inventory');
  }
  async function enterArchive(){await click('#recovery-nav summary',null,null);await click('#recovery-nav a','Archive','link');}
  switch(action){
    case 'life cancel': await open();await key('Escape','Escape',27);await wait(`!document.querySelector('dialog')`, 'cancelled');await grid(false);exchange();break;
    case 'life dialog':
      // The row menu may still be expanded after cancellation.
      if(await observe(`document.querySelector('${menu} .archive-item').hidden`)) await open(); else await click(menu+' .archive-item','Archive');
      await named('dialog','Archive selected items?');exchange();break;
    case 'life pending': await confirm();await wait(`document.querySelector('dialog [role=status]').innerText==='Saving…' && !document.querySelector('dialog button:enabled:not([hidden])')`, 'pending archive');await click(dlg+' button','Confirm','button',1,0,true);await grid(false);requests([post('archive')]);statuses();await command(1,1);break;
    case 'life fault': await confirm();await wait(`document.querySelector('dialog [role=status]').innerText==='Could not archive item.'`, 'commit failure');await grid(false);exchange([post('archive'),500]);await command(1,1);break;
    case 'life committed': case 'life retry': case 'life archive':
      if(action!=='life committed') await confirm();await grid(true);
      if(action==='life committed'){requests([get]);statuses([post('archive'),200],[get,200]);}else{exchange([post('archive'),200],[get,200]);await command(1,1,action==='life retry');}break;
    case 'life inspect': await enterArchive();await listing(true);exchange([inventory,200]);await historyAt(recovery,'push');break;
    case 'life reload': await reload();await listing(true);exchange([resolve,200],[inventory,200]);await historyAt(recovery);break;
    case 'life restoring': await restore();await wait(`document.querySelector('${row} button:disabled').innerText==='Restoring…'`, 'pending restore');requests([post('restore')]);statuses();await command(2,2);break;
    case 'life restored': await listing(false);requests([get,inventory]);statuses([post('restore'),200],[get,200],[inventory,200]);break;
    case 'life stale': await restore();await wait(`document.querySelector('#items').innerText.includes('Reload recovery list')`, 'conflict');exchange([post('restore'),409]);await command(2,2);break;
    case 'life review': await click('#items > button','Reload recovery list');await listing(true);exchange([inventory,200]);break;
    case 'life stale archive': await confirm();await wait(`document.querySelector('dialog [role=status]').innerText==='Item changed. Review latest items.'`, 'stale archive');await grid(false);exchange([post('archive'),409]);await command(4,3);break;
    case 'life review archive': await click('dialog button:nth-of-type(3)','Review latest items');await wait(`document.querySelector('dialog [role=status]').innerText==='Latest revisions loaded. Confirm to archive.' && !document.querySelector('dialog button').disabled`, 'archive revision review');exchange([get,200]);await key('Escape','Escape',27);await grid(false);exchange();break;
    case 'life latest restore': await restore();await listing(false);exchange([post('restore'),200],[get,200],[inventory,200]);await command(3,2);break;
    case 'life denied': await restore();await listing(true,false);exchange([post('restore'),403],[inventory,200]);await command(2,2);break;
    case 'life readonly': await reload();await listing(true,false);await click(row+' button','Restore item','button',1,0,true);exchange([resolve,200],[inventory,200]);break;
    case 'life member grid':case 'life viewer grid':{
      await click('#recovery-nav a','Close recovery','link');await grid(true);const activeRow=`[data-item-id="${uuid(61)}"]`;
      check(await observe(`!!document.querySelector('${activeRow} button.item-name')===${action==='life member grid'}`),'name edit capability');await click(activeRow+' .row-menu button','More options for Ink');await click(activeRow+' .archive-item','Archive','button',1,0,true);exchange([get,200]);await historyAt(path,'push');break;
    }
    case 'life return readonly': await enterArchive();await listing(true,false);exchange([inventory,200]);await historyAt(recovery,'push');break;
    case 'life unavailable': await enterArchive();await wait(`document.querySelector('main > [role=alert]').innerText==='Destination unavailable' && !document.querySelector('#items li')`, 'withdrawn workspace');exchange([inventory,403]);break;
    case 'life expired': await click('#retry-destination','Retry destination');await wait(`document.activeElement.id==='identifier' && !document.querySelector('#items li')`, 'expired recovery session');exchange([inventory,401]);break;
    case 'life close': await click('#recovery-nav a','Close recovery','link');await grid(false);exchange([get,200]);await historyAt(path,'push');await reload();await grid(false);exchange([resolve,200],[get,200]);await historyAt(path);break;
    case 'life empty': {
      await click('#back','Back to boards');await wait(`document.querySelector('[data-board-id="${uuid(202)}"]')`, 'board list');await click(`[data-board-id="${uuid(202)}"]`,'Review','link');await wait(`document.querySelector('#recovery-nav summary')`, 'second board');await enterArchive();await wait(`document.querySelector('#items p')?.innerText==='No archived items' && !document.querySelector('#items li')`, 'empty archive');exchange([['GET','/navigation/boards'],200],[selectedGet(202),200],[['GET',selectedGet(202)[1]+'/archive'],200]);break;
    }
    default:throw Error('Unknown lifecycle checkpoint');
  }
}
