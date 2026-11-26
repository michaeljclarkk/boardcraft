let a, b;
export async function groupCreateJourney(step, {browser,uuid,click,key,type,wait,check,requests,statuses,reload,selectedGet,resolve,observe,login,list,origin}) {
  const get = selectedGet(201), post = ['POST', get[1]+'/groups'];
  const exchange = (...pairs) => { requests(pairs.map(([r])=>r)); statuses(...pairs); };
  const eq = (expr,value,label) => wait(`JSON.stringify(${expr})===${JSON.stringify(JSON.stringify(value))}`,label);
  const toggle='#new-item-options summary', create='#new-item-options details button';
  const blue='rgb(87, 155, 252)', green='rgb(0, 200, 117)';
  async function grid(stage) {
    const group=(id,title,ids=[],colour=blue)=>[id,`${title} / ${ids.length?'1 item':'No Items'}`,ids.map(n=>[uuid(n),'Ink']),colour];
    const shape=[group('g-old','New group',[59]),...(stage?[group(a,'New group')]:[]),group('g-z','Work',[61]),
      ...(stage===3?[group('g-'+uuid(800),'Peer'),group(b,'Work',[],green)]:[]),group('g-end','Reserve')];
    await eq(`[...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,s.querySelector('h2').innerText,[...s.querySelectorAll('li')].map(r=>[r.dataset.itemId,r.querySelector('.item-name').innerText]),getComputedStyle(s.querySelector('h2')).borderLeftColor])`,shape,'ordered group headings colours and roots');
  }
  async function dialogButton(label,disabled=false) {
    const index=await observe(`[...document.querySelectorAll('dialog button')].findIndex(b=>b.innerText===${JSON.stringify(label)} && !b.hidden)`);
    check(index>=0,'visible dialog action');await click(`dialog button:nth-of-type(${index+1})`,label,'button',1,0,disabled);
  }
  const name=`document.querySelector('dialog input')`, colour=`document.querySelector('dialog select')`;
  async function open() {
    await click(toggle,null,null);await click(create,'New group of items');
    await wait(`document.querySelector('dialog[open]').getAttribute('aria-label')==='Add group' && ${name}===document.activeElement && ${name}.value==='New group' && ${name}.selectionStart===9 && ${name}.selectionEnd===9 && ${colour}.value==='#579bfc'`,'focused default and caret');
  }
  async function payload(rev,edited=false) {
    check(browser.createIds.length===1,'one group command');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.createIds.shift()});
    const d=JSON.parse(postData);
    check(/^g-[0-9a-f-]{36}$/.test(d.id) && d.board_revision===rev && d.name===(edited?'  Work  ':'New group') && d.colour===(edited?'#00c875':'#579bfc'),'submitted identity revision raw name and colour');
    return d.id;
  }
  const closed=()=>wait(`!document.querySelector('dialog,#items h2 input')`,'no second editor');
  const retained=()=>wait(`${name}.value==='  Work  ' && ${colour}.value==='#00c875'`,'retained draft');
  switch(step) {
    case 'open':
      statuses([login,200],[resolve,200],[list,200]);await browser.send('Emulation.setDeviceMetricsOverride',{width:1100,height:1800,deviceScaleFactor:1,mobile:false});
      await click(`[data-board-id="${uuid(201)}"]`,'Review','link');await grid(0);exchange([get,200]);break;
    case 'cancel':
      await open();await key('a','KeyA',65,2);await type('   ');await dialogButton('Save group',true);await key('Escape','Escape',27);await closed();
      await open();await dialogButton('Cancel');await closed();await grid(0);exchange();break;
    case 'dialog':await open();exchange();break;
    case 'pending':
      await dialogButton('Save group');await wait(`document.querySelector('dialog [role=status]').innerText==='Saving…'`,'pending group');await dialogButton('Save group',true);await key('Escape','Escape',27);
      await grid(0);requests([post]);statuses();a=await payload(1);return a.slice(2);
    case 'committed':await closed();await grid(1);requests([get]);statuses([post,200],[get,200]);break;
    case 'default':await dialogButton('Save group');await closed();a=await payload(1);await grid(1);exchange([post,200],[get,200]);return a.slice(2);
    case 'reload default':case 'reload green':await reload();await grid(step==='reload green'?3:1);await closed();exchange([resolve,200],[get,200]);break;
    case 'edit':await open();await key('a','KeyA',65,2);await type('  Work  ');await key('Tab','Tab',9);await key('End','End',35);await retained();await grid(1);exchange();break;
    case 'fault':await dialogButton('Save group');await wait(`document.querySelector('dialog [role=status]').innerText==='Could not save group.' && document.activeElement===${name}`,'recoverable refusal');await retained();await grid(1);exchange([post,500]);b=await payload(2,true);return b.slice(2);
    case 'conflict':await dialogButton('Save group');await wait(`document.activeElement.innerText==='Review latest board'`,'conflict');await retained();await grid(1);exchange([post,409]);check(await payload(2,true)===b,'same retry identity');break;
    case 'review':await dialogButton('Review latest board');await wait(`document.querySelector('dialog [role=status]').innerText==='Board revision 3: New group, New group, Work, Peer, Reserve'`,'review accepted names and revision');await retained();exchange([get,200]);break;
    case 'adopt':await dialogButton('Keep my draft and use this revision');await wait(`document.activeElement===${name}`,'explicit adoption');await retained();exchange();break;
    case 'refresh fault':await browser.failGroupRead(origin+'/preview/api'+get[1]);await dialogButton('Save group');await wait(`document.querySelector('dialog [role=status]').innerText==='Could not read the latest board.' && ![...document.querySelectorAll('dialog button')].some(b=>b.textContent==='Save group' && !b.hidden)`,'read only recovery');exchange([post,200],[get,503]);check(await payload(3,true)===b,'adopted baseline and identity');break;
    case 'read accepted':await browser.restoreGroupRead();await dialogButton('Reload board');await closed();await grid(3);exchange([get,200]);break;
    case 'denied':await dialogButton('Save group');await wait(`document.querySelector('main > [role=alert]').innerText==='Group creation denied.' && document.querySelector('${create}').disabled`,'authorized reread');await closed();await grid(0);exchange([post,403],[get,200]);await payload(1);break;
    case 'member':case 'viewer':
      if(step==='viewer'){await reload();await grid(0);exchange([resolve,200],[get,200]);}
      await click(toggle,null,null);await click(create,'New group of items','button',1,0,true);await closed();check(await observe(`!!document.querySelector('.add-item button:enabled')`)===(step==='member'),'ordinary item creation');exchange();break;
    case 'withdrawn':await reload();await wait(`document.querySelector('main > [role=alert]').innerText==='Destination unavailable' && !document.querySelector('#items section')`,'withdrawn access');exchange([resolve,200],[get,403]);break;
    case 'expired':await click('#retry-destination','Retry destination');await wait(`document.activeElement.id==='identifier' && !document.querySelector('#items section')`,'expired session');exchange([get,401]);break;
    default:throw Error('unknown group create checkpoint');
  }
}
