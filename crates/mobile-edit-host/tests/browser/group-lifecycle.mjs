export async function groupLifecycleJourney(action,{browser,uuid,click,key,wait,check,requests,statuses,reload,historyAt,traverse,boardPath,selectedGet,resolve,observe,login,list,origin}) {
  const get=selectedGet(201), inventory=['GET',get[1]+'/archive'];
  const post=a=>['POST',get[1]+'/groups/g-90/'+a];
  const exchange=(...pairs)=>{requests(pairs.map(([req])=>req));statuses(...pairs);};
  const match=async(expr,expected,label)=>wait(`JSON.stringify(${expr})===${JSON.stringify(JSON.stringify(expected))}`,label);
  async function grid(active) {
    const shape=[['g-20','Work / 1 item',[uuid(59)]],...(active?[['g-90','Work / 3 items',[73,61,77].map(uuid)]]:[]),['g-10','Reserve / No Items',[]]];
    await match(`[...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,s.querySelector('h2').innerText,[...s.querySelectorAll('li')].map(r=>r.dataset.itemId)])`,shape,'active groups and children');
  }
  async function listing(active,owner=true){
    if (!owner) await wait(`document.querySelector('#items').innerText.includes('Owner access required.')`,'authorized inventory');
    const groups=[['g-30','Retired',!owner],...(active?[]:[['g-90','Work',!owner]])];
    await match(`[...document.querySelectorAll('#items li[data-group-id]')].map(g=>[g.dataset.groupId,g.firstChild.innerText,g.querySelector('button').disabled])`,groups,'archived physical groups');
    const ids=active?[57,67]:[57,73,69,61,67,77];
    await match(`[...document.querySelectorAll('#items li[data-item-id]')].map(i=>[i.dataset.itemId,i.querySelector('button').disabled,i.innerText.includes('Restore its group first.')])`,ids.map(n=>[uuid(n),!active||n===57||!owner,!active||n===57]),'effective archive');
  }
  async function baseline(b){
    check(browser.lifecycleIds.length===1,'one group command');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.lifecycleIds.shift()});
    check(JSON.stringify(JSON.parse(postData))===JSON.stringify({board_revision:b}),'only expected board revision');
  }
  const menu=(id='g-90')=>click(`section[data-group-id="${id}"] .group-menu button`,'Group menu for Work');
  const archive=()=>click('section[data-group-id="g-90"] .group-menu button:last-child','Archive group');
  const restore=()=>click('li[data-group-id="g-90"] button','Restore group');
  const toast=()=>wait(`document.querySelector('.group-notice')?.innerText.includes('Work was removed.') && document.querySelector('.group-notice button')?.innerText==='Undo'`,'transient Undo');
  const open=async()=>{await click('#recovery-nav summary',null,null);await click('#recovery-nav a','Archive','link');};
  const board=boardPath(201);
  switch(action){
    case 'open':
      statuses([login,200],[resolve,200],[list,200]);await browser.send('Emulation.setDeviceMetricsOverride',{width:1100,height:1400,deviceScaleFactor:1,mobile:false});await click(`[data-board-id="${uuid(201)}"]`,'Review','link');await grid(true);exchange([get,200]);await historyAt(board,'push');break;
    case 'escape': await menu();await key('Escape','Escape',27);await wait(`document.activeElement.matches('[data-group-id="g-90"] .group-menu [aria-expanded=false]')`,'closed group menu');exchange();break;
    case 'pending':await menu();await archive();await grid(true);await menu();await wait(`document.querySelector('[data-group-id="g-90"] .group-menu button:last-child').hidden && !document.querySelector('dialog,.group-notice button')`,'pending archive');requests([post('archive')]);statuses();await baseline(2);break;
    case 'committed':await toast();await grid(false);requests([get,inventory]);statuses([post('archive'),200],[get,200],[inventory,200]);break;
    case 'archive':case 'again':await menu();await archive();await toast();await grid(false);exchange([post('archive'),200],[get,200],[inventory,200]);await baseline(action==='again'?4:2);break;
    case 'fault':await menu();await archive();await wait(`document.querySelector('.group-notice').innerText.includes('Could not change group') && !document.querySelector('.group-notice button')`,'failed archive');await grid(true);exchange([post('archive'),500]);await baseline(2);break;
    case 'undo':await click('.group-notice button','Undo');await grid(true);exchange([post('restore'),200],[get,200],[inventory,200]);await baseline(3);break;
    case 'dismiss':await click('.group-notice button:last-child','Dismiss');await grid(false);exchange();break;
    case 'inspect':await open();await listing(false);exchange([inventory,200]);await historyAt(board+'/archive','push');break;
    case 'back':await traverse(-1);await grid(false);check(!await observe(`!!document.querySelector('.group-notice button')`),'history did not recreate Undo');exchange([get,200]);await historyAt(board,'back');break;
    case 'forward':await traverse(1);await listing(false);exchange([inventory,200]);await historyAt(board+'/archive','forward');break;
    case 'reload recovery':await reload();await listing(false);exchange([resolve,200],[inventory,200]);await historyAt(board+'/archive');break;
    case 'restore':await restore();await listing(true);exchange([post('restore'),200],[get,200],[inventory,200]);await baseline(5);break;
    case 'close':await click('#recovery-nav a','Close recovery','link');await grid(true);exchange([get,200]);await reload();await grid(true);check(!await observe(`!!document.querySelector('.group-notice button')`),'no Undo after reload');exchange([resolve,200],[get,200]);break;
    case 'stale':await restore();await wait(`document.querySelector('#items > button')?.innerText==='Reload recovery list'`,'stale restore');exchange([post('restore'),409]);await baseline(3);break;
    case 'review':await click('#items > button','Reload recovery list');await listing(false);exchange([inventory,200]);break;
    case 'refresh fault':
      await browser.failGroupRead(origin+'/preview/api'+get[1]);await restore();await wait(`document.querySelector('#items').innerText.includes('Restore saved.')`,'failed refresh after commit');exchange([post('restore'),200],[get,503]);await baseline(4);break;
    case 'read accepted':await browser.restoreGroupRead();await click('#items > button','Reload recovery list');await listing(true);exchange([inventory,200]);break;
    case 'denied':await restore();await listing(false,false);exchange([post('restore'),403],[inventory,200]);await baseline(3);break;
    case 'readonly':await reload();await listing(false,false);await click('li[data-group-id="g-90"] button','Restore group','button',1,0,true);exchange([resolve,200],[inventory,200]);break;
    case 'member':case 'viewer':await click('#recovery-nav a','Close recovery','link');await grid(false);await menu('g-20');await click('[data-group-id="g-20"] .group-menu button:last-child','Archive group','button',1,0,true);check(await observe(`!!document.querySelector('button.item-name:enabled')`) === (action==='member'),'ordinary name edit');exchange([get,200]);break;
    case 'inspect readonly':await open();await listing(false,false);exchange([inventory,200]);break;
    default:throw Error('Unknown group checkpoint');
  }
}
