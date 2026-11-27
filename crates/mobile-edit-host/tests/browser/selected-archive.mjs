let operation;
export async function selectedArchiveJourney(step,{browser,uuid,click,key,wait,check,requests,statuses,reload,selectedGet,resolve,observe,named},trash=false) {
  const verb=trash?'Delete':'Archive', place=trash?'Trash':'Archive', selected=trash?[67,59]:[67,59,61], peer=trash?59:61;
  const get=selectedGet(201), post=['POST',get[1]+'/items/'+(trash?'delete-selection':'archive-selection')], inventory=['GET',get[1]+(trash?'/trash':'/archive')];
  const tools='[role=toolbar]', dialog='dialog[open]', archive=tools+(trash?' .delete-selected':' .archive-selected');
  const exchange=(...pairs)=>{requests(pairs.map(([r])=>r));statuses(...pairs);};
  const toggle=n=>click(`[data-item-id="${uuid(n)}"] input`,'Select Ink','checkbox');
  const open=async()=>{await click(archive,verb);await named('dialog',verb+' selected items?');await wait(`document.querySelector('dialog p').textContent.includes('${trash?'2 items: Ink, Ink':'3 items: Ink, Ink, Ink'}') && document.querySelector('dialog p').textContent.includes('${place}')`,'selected lifecycle summary');};
  const confirm=()=>click(dialog+' button',trash?'Delete':'Confirm');
  const review=()=>click(dialog+' button:nth-of-type(3)','Review latest items');
  async function payload(repeat=false,changed=false) {
    check(browser.batchIds.length===1,'one selected archive');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.batchIds.shift()});const body=JSON.parse(postData);
    check(Object.keys(body).sort().join()==='board_revision,items,operation_id' && body.board_revision===2 && JSON.stringify(body.items)===JSON.stringify(selected.map(n=>({id:uuid(n),revision:changed&&n===peer?2:1}))), 'selected lifecycle baselines');
    check(/^[0-9a-f-]{36}$/.test(body.operation_id) && (repeat ? body.operation_id===operation : body.operation_id!==operation),'selected archive identity');operation=body.operation_id;
  }
  async function grid(archived=false,restored=false,folded=true) {
    const shape=[['g-90',archived?(trash?(restored?[73,61,67,77]:[73,61,77]):(restored?[73,67,77]:[73,77])):[73,61,67,77]],['g-10',[]],['g-20',folded?[]:[59]]].map(([id,ids])=>[id,ids.map(uuid)]);
    await wait(`JSON.stringify([...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,[...s.querySelectorAll('li')].map(r=>r.dataset.itemId)]))===${JSON.stringify(JSON.stringify(shape))}`,'archive grid');
    if(folded) check(await observe(`document.querySelector('[data-group-id="g-20"] .fold-toggle').getAttribute('aria-expanded')==='false'`),'fold retained');
  }
  switch(step) {
    case 'prepare':
      operation=undefined;for(const n of selected)await toggle(n);
      await click('[data-group-id="g-20"] .fold-toggle','Toggle Work');
      await grid();await wait(`document.querySelector('${tools} span').textContent==='${selected.length} selected'`,'hidden selection');
      exchange([['POST',get[1]+'/main-table-fold'],200],[get,200]);break;
    case 'readonly':
      for(const n of selected)await toggle(n);await click(archive,verb,'button',1,0,true);check(await observe(`!document.querySelector('dialog') && document.querySelector('${tools} span').textContent==='${selected.length} selected'`),'readonly selection');exchange();break;
    case 'cancel':
      await open();await key('Escape','Escape',27);await wait(`!document.querySelector('dialog')`,'archive cancelled');await grid();check(await observe(`document.activeElement.matches('${archive}')`),'archive trigger focus');await open();exchange();break;
    case 'open':await open();exchange();break;
    case 'pending':
      await confirm();await wait(`document.querySelector('dialog [role=status]').textContent==='Saving…' && !document.querySelector('dialog button:enabled:not([hidden])')`,'archive pending');await click(dialog+' button',trash?'Delete':'Confirm','button',1,0,true);await key('Escape','Escape',27);await grid();requests([post]);statuses();await payload();break;
    case 'saved':
      await grid(true);await wait(`!document.querySelector('dialog,[role=toolbar]')`,'archived selection cleared');requests([get]);statuses([post,200],[get,200]);return operation;
    case 'conflict':case 'fault':
      await confirm();await wait(`document.querySelector('dialog [role=status]').textContent==='${step==='conflict'?'Items changed. Review latest items.':verb+' not confirmed. Review latest items.'}'`,'archive refusal');await grid();exchange([post,step==='conflict'?409:500]);await payload();break;
    case 'failed review':
      await browser.failGroupRead(new URL(await observe('location.href')).origin+'/preview/api'+get[1]);await review();await wait(`document.querySelector('dialog [role=status]').textContent==='Could not review selected items.'`,'review failed with retained choices');exchange([get,503]);await browser.restoreGroupRead();break;
    case 'review':case 'missing review': {
      await review();const missing=step==='missing review';
      await wait(`document.querySelector('dialog [role=status]').textContent==='Review every requested item before adopting these revisions.' && document.querySelector('dialog button:last-child').disabled===${missing}`,'review complete');
      const details=selected.map(n=>missing&&n===peer?`${uuid(n)}: Unavailable`:`${n===peer?'Indigo':'Ink'} (${uuid(n)}), revision ${n===peer?2:1}`);
      check(await observe(`JSON.stringify([...document.querySelectorAll('dialog div p')].map(p=>p.textContent))===${JSON.stringify(JSON.stringify(details))}`),'all original IDs reviewed');exchange([get,200]);break;
    }
    case 'review fault':
      await review();await wait(`!document.querySelector('dialog button:last-child').hidden && !document.querySelector('dialog button:last-child').disabled`,'unchanged targets reviewed');exchange([get,200]);break;
    case 'adopt':
      await click('dialog button:last-child','Use these revisions and keep my choices');await wait(`!document.querySelector('dialog button').disabled`,'separate confirmation required');exchange();break;
    case 'confirmed':case 'confirmed fault':
      await confirm();await grid(true);exchange([post,200],[get,200]);await payload(true,step==='confirmed');return operation;
    case 'inspect':
      await click('#recovery-nav summary',null,null);await click(trash?'#recovery-nav a:nth-of-type(2)':'#recovery-nav a',place,'link');
      await wait(`JSON.stringify([...document.querySelectorAll('#items li[data-item-id]')].map(r=>r.dataset.itemId))===${JSON.stringify(JSON.stringify((trash?[68,59,67]:[57,59,61,67]).map(uuid)))}`,'selected recovery inventory');exchange([inventory,200]);
      if(trash){ await reload();await wait(`document.querySelector('#items p')?.textContent==='3 deleted items'`,'trash direct reopen');exchange([resolve,200],[inventory,200]); }
      break;
    case 'restore':
      await click(`[data-item-id="${uuid(67)}"] button`,'Restore item');await wait(`document.querySelector('#items p').textContent==='${trash?'2 deleted':'3 archived'} items' && !document.querySelector('[data-item-id="${uuid(67)}"]') && !!document.querySelector('[data-item-id="${uuid(59)}"]') && !!document.querySelector('[data-item-id="${uuid(trash?68:61)}"]')`,'only one root restored');exchange([['POST',get[1]+`/items/${uuid(67)}/${trash?'restore-deleted':'restore'}`],200],[get,200],[inventory,200]);break;
    case 'reopen':
      await click('#recovery-nav a','Close recovery','link');await grid(true,true);exchange([get,200]);await reload();await grid(true,true);exchange([resolve,200],[get,200]);break;
    case 'denied':
      await confirm();await wait(`!document.querySelector('dialog') && document.querySelector('${archive}').disabled`,'current Member cannot archive');await grid();exchange([post,403],[get,200]);await payload();await click(archive,verb,'button',1,0,true);exchange();break;
    case 'no access':
      await confirm();await wait(`document.querySelector('main > [role=alert]').textContent==='Destination unavailable' && !document.querySelector('#items li')`,'withdrawn access');exchange([post,403],[get,403]);await payload();break;
    case 'expired':
      await confirm();await wait(`document.activeElement.id==='identifier' && !document.querySelector('#items li')`,'archive session expired');exchange([post,401]);await payload();break;
    case 'uncertain ack':case 'uncertain refresh': {
      const url=new URL(await observe('location.href')).origin+'/preview/api';
      if(step==='uncertain ack')await browser.failFoldConfirmation(url+post[1]);else await browser.failGroupRead(url+get[1]);
      await confirm();await wait(`!document.querySelector('dialog,#items li') && !document.querySelector('#retry-destination').hidden`,'archive read barrier');
      exchange(...(step==='uncertain ack'?[[post,503]]:[[post,200],[get,503]]));await payload();await browser.restoreGroupRead();return operation;
    }
    case 'reconcile':
      await click('#retry-destination','Retry destination');await grid(true);exchange([get,200]);break;
    default:throw Error('unknown selected archive checkpoint');
  }
}
