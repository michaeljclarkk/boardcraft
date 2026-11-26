export async function groupEditJourney(step, {browser,uuid,click,key,type,wait,check,requests,statuses,reload,selectedGet,resolve,observe,login,list,origin}) {
  const get = selectedGet(201), put = ['PUT', get[1]+'/groups/g-z'];
  const exchange = (...pairs) => {requests(pairs.map(([r]) => r)); statuses(...pairs);};
  const section = 'section[data-group-id="g-z"]', title = section+' h2 span', menu = section+' .group-menu button';
  const name = `document.querySelector('dialog input')`, colour = `document.querySelector('dialog select')`;
  const stored='#443322', blue='#579bfc', green='#00c875', raw='  Reserve  ';
  const status = `document.querySelector('dialog [role=status]').textContent`;
  const closed = () => wait(`!document.querySelector('dialog,#items h2 input')`,'editor dismissed');
  const retained = () => wait(`${name}.value===${JSON.stringify(raw)} && ${colour}.value==='${green}'`,'raw fields preserved');
  const input = async text => { await key('a','KeyA',65,2); await type(text); };
  const tab = () => key('Tab','Tab',9);
  async function grid(label='Work',hex=stored,active=true) {
    const rgb = h => 'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';
    const row = (id,label,ids,h=stored) => [id,label+' / '+(['No Items','1 item','2 items'][ids.length]),rgb(h),ids.map(i=>[uuid(i),'Ink'])];
    const expected=[row('g-old','Work',[59]),...(active?[row('g-z',label,[61,63],hex)]:[]),row('g-end','Reserve',[])];
    await wait(`JSON.stringify([...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,s.querySelector('h2').innerText,getComputedStyle(s.querySelector('h2')).borderLeftColor,[...s.querySelectorAll('li')].map(r=>[r.dataset.itemId,r.querySelector('.item-name').innerText])]))===${JSON.stringify(JSON.stringify(expected))}`,'ordered groups, colours, IDs, counts and children');
  }
  async function open(via,label='Work',hex=stored) {
    if (via==='heading') await click(title,null,null,2);
    else {
      await click(menu,`Group menu for ${label}`);
      await click(menu+`:nth-child(${via==='rename'?2:3})`,via==='rename'?'Rename group':'Change group colour');
    }
    await wait(`document.querySelector('dialog[open]').getAttribute('aria-label')==='Edit group' && !document.querySelector('#items h2 input') && ${name}===document.activeElement && ${name}.value===${JSON.stringify(label)} && ${name}.selectionStart===${label.length} && ${name}.selectionEnd===${label.length} && ${colour}.value==='${hex}'`,'focused accepted fields and end caret');
  }
  async function button(label,disabled=false){
    const index=await observe(`[...document.querySelectorAll('dialog button')].findIndex(b=>b.textContent===${JSON.stringify(label)} && !b.hidden)`);
    check(index>=0,'visible '+label); await click(`dialog button:nth-of-type(${index+1})`,label,'button',1,0,disabled);
  }
  async function payload(board_revision,group_revision,name,colour) {
    check(browser.editGroupIds.length===1,'one edit of g-z');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.editGroupIds.shift()});
    const d=JSON.parse(postData), expected={board_revision,group_revision,name,colour};
    check(Object.keys(d).length===4 && Object.keys(expected).every(k=>d[k]===expected[k]),'both raw fields and revision baselines');
  }
  switch(step) {
    case 'open':statuses([login,200],[resolve,200],[list,200]); await browser.send('Emulation.setDeviceMetricsOverride',{width:1100,height:1800,deviceScaleFactor:1,mobile:false});await click(`[data-board-id="${uuid(201)}"]`,'Review','link');await grid();exchange([get,200]);break;
    case 'cancel':await click(title,null,null);await closed();await open('heading');await input('   ');await tab();await button('Save group',true);await key('Escape','Escape',27);await closed();await grid();await open('colour');await tab();await key('g','KeyG',71,0,'g');await wait(`${colour}.value==='${green}'`,'green choice');await button('Cancel');await closed();await grid();exchange();break;
    case 'rename':await open('rename');exchange();break;
    case 'pending':await button('Save group');await wait(`${status}==='Saving…'`,'pending save');await button('Save group',true);await button('Cancel',true);await key('Escape','Escape',27);await wait(`!!document.querySelector('dialog[open]')`,'pending dialog retained');await grid();requests([put]);statuses();await payload(7,1,'Work',stored);break;
    case 'committed':await closed();await grid();requests([get]);statuses([put,200],[get,200]);break;
    case 'unchanged':await open('rename');await button('Save group');await closed();await grid();exchange([put,200],[get,200]);await payload(7,1,'Work',stored);break;
    case 'draft':await open('heading');await input(raw);await tab();await key('g','KeyG',71,0,'g');await retained();exchange();break;
    case 'refusal':await button('Save group');await wait(`${status}==='Could not save group.' && ${name}===document.activeElement`,'correctable rejection');await retained();await grid();exchange([put,500]);await payload(8,2,raw,green);break;
    case 'conflict':case 'archived conflict':await button('Save group');await wait(`document.activeElement.textContent==='Review latest board'`,'conflict requires review');exchange([put,409]);if(step==='conflict'){await retained();await payload(8,2,raw,green);}else await payload(12,6,'Ink',blue);break;
    case 'review':await button('Review latest board');await wait(`${status}==='Board revision 9, group g-z revision 3: Peer / #abcdef'`,'review accepted group and both revisions');await retained();exchange([get,200]);break;
    case 'adopt':await button('Keep my draft and use this revision');await wait(`${name}===document.activeElement`,'adoption only');await retained();exchange();break;
    case 'refresh fault':await browser.failGroupRead(origin+'/preview/api'+get[1]);await button('Save group');await wait(`${status}==='Could not read the latest board.' && ![...document.querySelectorAll('dialog button')].some(b=>b.textContent==='Save group' && !b.hidden)`,'only a read may retry');exchange([put,200],[get,503]);await payload(9,3,raw,green);break;
    case 'read accepted':await browser.restoreGroupRead();await button('Reload board');await closed();await grid('Reserve',green);await wait(`document.querySelector('.group-notice').textContent==='Board revision 10, group g-z revision 4: Reserve / #00c875'`,'accepted state after reload');exchange([get,200]);break;
    case 'reload green':case 'reload blue':await reload();await closed();await grid(step==='reload green'?'Reserve':'Ink',step==='reload green'?green:blue);exchange([resolve,200],[get,200]);break;
    case 'name only':case 'name green':{
      const reviewed=step==='name green', hex=reviewed?green:stored;
      await open('rename',reviewed?'Reserve':'Work',hex);await input('  Ink  ');await key('Enter','Enter',13);await closed();await grid('Ink',hex);exchange([put,200],[get,200]);await payload(reviewed?10:8,reviewed?4:2,'  Ink  ',hex);break;
    }
    case 'colour only':case 'colour blue':{
      const reviewed=step==='colour blue';await open('colour','Ink',reviewed?green:stored);await tab();await key('Home','Home',36);await button('Save group');await closed();await grid('Ink',blue);exchange([put,200],[get,200]);await payload(reviewed?11:9,reviewed?5:3,'Ink',blue);break;
    }
    case 'open ink':await open('rename','Ink',blue);exchange();break;
    case 'archived review':await button('Review latest board');await closed();await grid(null,null,false);await wait(`document.querySelector('.group-notice').textContent==='Group g-z is no longer available.'`,'missing target cannot be adopted');exchange([get,200]);break;
    case 'denied':await button('Save group');await wait(`document.querySelector('main > [role=alert]').textContent==='Group edit denied.' && document.querySelector('#new-item-options button').disabled`,'authorized reread');await closed();await grid();exchange([put,403],[get,200]);await payload(7,1,'Work',stored);break;
    case 'member':case 'viewer':if(step==='viewer'){await reload();await grid();exchange([resolve,200],[get,200]);}await click(title,null,null,2);await closed();await click(menu,'Group menu for Work');await click(menu+':nth-child(2)','Rename group','button',1,0,true);await click(menu+':nth-child(3)','Change group colour','button',1,0,true);await closed();check(await observe(`!!document.querySelector('button.item-name:enabled')`)===(step==='member'),'ordinary item permissions');exchange();break;
    default:throw Error('unknown edit checkpoint');
  }
}
