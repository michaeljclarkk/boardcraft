let operation;
export async function batchJourney(action, {browser,uuid,click,key,wait,check,requests,statuses,reload,historyAt,boardPath,selectedGet,resolve,observe,named}) {
  const get = selectedGet(201), post = ['POST', get[1]+'/items/move-selection'];
  const bar = '[role=toolbar]', dlg = 'dialog[open]', initial = [[73,61,67,77],[],[59]], order = [73,77,67,59,61];
  const names = {73:'Sketch',61:'Ink',67:'Ink',77:'Fold',59:'Ink'};
  const toggle = n => click(`[data-item-id="${uuid(n)}"] input`, 'Select '+names[n],'checkbox');
  const move = () => click(bar+' button:last-child','Move to');
  const confirm = () => click(dlg+' button','Confirm');
  const review = () => click(dlg+' button:nth-of-type(3)','Review latest items');
  const adopt = () => click(dlg+' button:last-child','Use these revisions and keep my choices');
  async function grid(groups, checked, viewer=false) {
    const shape = groups.map((ids,i)=> {
      const n=ids.filter(id=>checked.includes(id)).length;
      return [['g-90','g-10','g-20'][i],['Work','Reserve','Work'][i]+' / '+(!ids.length?'No Items':ids.length+(ids.length===1?' item':' items')),
        !!n && n===ids.length,!!n && n<ids.length,ids.map(id=>[uuid(id),checked.includes(id)])];
    });
    await wait(`JSON.stringify([...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,s.querySelector('h2').innerText,s.firstChild.checked,s.firstChild.indeterminate,[...s.querySelectorAll('li')].map(r=>[r.dataset.itemId,r.firstChild.checked])]))===${JSON.stringify(JSON.stringify(shape))} &&
      (${checked.length} ? document.querySelector('${bar} span')?.innerText==='${checked.length} selected' && document.querySelector('${bar} button:last-child').disabled===${viewer} : !document.querySelector('${bar}'))`, 'batch grid, checks and toolbar');
  }
  async function dialog(summary, group='g-90') {
    await named('dialog','Move selected items'); await named('combobox','Destination group');
    check(await observe(`document.querySelector('dialog p').textContent===${JSON.stringify(summary)} && document.querySelector('dialog select:disabled').value==='${uuid(201)}' && document.querySelector('dialog select:last-of-type:not(:disabled)').value==='${group}' && JSON.stringify([...document.querySelector('dialog label:last-of-type select').options].map(o=>[o.value,o.text]))==='[["g-90","Work"],["g-10","Reserve"],["g-20","Work"]]'`),'dialog contents');
  }
  async function payload(ids=[67,59,61], revisions=[1,1,1], group='g-90', rev=1, repeat=false) {
    check(browser.batchIds.length===1,'one batch');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.batchIds.shift()}); const body=JSON.parse(postData);
    check(/^[0-9a-f-]{36}$/.test(body.operation_id) && (repeat?body.operation_id===operation:body.operation_id!==operation),'operation identity');
    operation=body.operation_id;
    check(body.group_id===group && body.board_revision===rev && JSON.stringify(body.items)===JSON.stringify(ids.map((id,i)=>({id:uuid(id),revision:revisions[i]}))),'ordered batch baselines');
  }
  const exchange = (...pairs) => {requests(pairs.map(([r])=>r)); statuses(...pairs);};
  const checks=[61,67,59], summary='3 selected: Ink, Ink, Ink';
  switch(action) {
    case 'batch open':
      for (const n of [67,59,61]) await toggle(n); await grid(initial,checks); await move(); await dialog(summary); exchange(); break;
    case 'batch cancel':
      await key('End','End',35); await dialog(summary,'g-20'); await key('Escape','Escape',27); await wait(`!document.querySelector('dialog')`,'cancel');
      await grid(initial,checks); await move(); await dialog(summary); exchange(); break;
    case 'batch pending':
      await confirm(); await wait(`document.querySelector('dialog [role=status]').innerText==='Saving…' && !document.querySelector('dialog button:enabled,dialog select:enabled')`, 'busy dialog');
      await click(dlg+' button','Confirm','button',1,0,true); await key('Escape','Escape',27); await grid(initial,checks); requests([post]);statuses();await payload();break;
    case 'batch saved': case 'batch reviewed saved':
      if (action==='batch reviewed saved') await confirm(); await grid([order,[],[]],[]); await wait(`!document.querySelector('dialog')`, 'closed after read');
      if (action==='batch saved') {requests([get]);statuses([post,200],[get,200]);}
      else {exchange([post,200],[get,200]);await payload(undefined,[1,1,2],undefined,1,true); check(await observe(`document.querySelector('[data-item-id="${uuid(61)}"] .item-name').innerText==='Indigo'`),'peer rename retained');} break;
    case 'batch reload':
      await reload(); await grid([order,[],[]],[]); exchange([resolve,200],[get,200]); break;
    case 'batch reserve':
      await toggle(67); await toggle(73); await click('[data-group-id="g-90"] > input','Select all items in Work','checkbox'); await grid([order,[],[]],order);
      await move(); await dialog('5 selected: Sketch, Fold, Ink, Ink, Ink'); await key('ArrowDown','ArrowDown',40); await confirm(); await grid([[],order,[]],[]);
      exchange([get,200],[post,200],[get,200]); await payload(order,[1,1,2,2,2],'g-10',2); break;
    case 'batch conflict': case 'batch fault':
      await confirm(); await wait(`document.querySelector('dialog [role=status]').innerText==='${action==='batch conflict'?'Selection changed. Review latest items.':'Could not save move.'}'`, 'retained failed command');
      await dialog(summary); await grid(initial,checks); exchange([post,action==='batch conflict'?409:500]);
      await payload(undefined,action==='batch fault'?[1,1,2]:undefined,undefined,1,action==='batch fault');break;
    case 'batch unavailable': case 'batch review': {
      const missing=action==='batch unavailable'; await review();
      const details = [67,59,61].map(n=>missing && n===61 ? `${uuid(n)}: Unavailable` : `${n===61?'Indigo':'Ink'} (${uuid(n)}), revision ${n===61?2:1}`);
      await wait(`JSON.stringify([...document.querySelectorAll('dialog div p')].map(p=>p.innerText))===${JSON.stringify(JSON.stringify(details))} && document.querySelector('dialog button:last-child').disabled===${missing} && document.querySelector('dialog button').disabled`, 'review in request order');
      exchange([get,200]); if (!missing) {await adopt(); await wait(`!document.querySelector('dialog button').disabled`, 'explicit confirm required'); exchange();} break;
    }
    case 'batch choose other': await key('End','End',35);await dialog(summary,'g-20');exchange();break;
    case 'batch denied':
      await confirm(); await wait(`!document.querySelector('dialog,#items button.item-name')`, 'demotion'); await grid(initial,checks,true);exchange([post,403],[get,200]);await payload(undefined,undefined,'g-20');
      await click(bar+' button:last-child','Move to','button',1,0,true); await click(bar+' button','Clear item selection');await grid(initial,[],true);exchange();break;
    case 'batch viewer':
      await reload();await grid(initial,[],true);exchange([resolve,200],[get,200]); await toggle(67);await grid(initial,[67],true);await click(bar+' button:last-child','Move to','button',1,0,true);exchange();break;
    default: throw Error('unknown batch checkpoint');
  }
  await historyAt(boardPath(201));
}
