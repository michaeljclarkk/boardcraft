const A='g-old', B='g-z', C='g-end', ABC=[A,B,C], BAC=[B,A,C];
const children = {[A]:[59],[B]:[61,63],[C]:[]};
const name = id => id===C?'Reserve':'Work';
const section = id => `[data-group-id="${id}"]`;

export async function foldJourney(step, {browser,uuid,click,key,wait,check,requests,statuses,reload,selectedGet,resolve,observe,list,origin,choose,password,passwordStep,identifierScreen,type,enter,login}) {
  const get=selectedGet(201), post=['POST',get[1]+'/main-table-fold'], order=['POST',get[1]+'/group-order'];
  const exchange=(...pairs)=>{requests(pairs.map(([r])=>r));statuses(...pairs);};
  const row=n=>`li[data-item-id="${uuid(n)}"] input`;
  const toggle=id=>click(section(id)+' .fold-toggle',`Toggle ${name(id)}`);
  const notice=`document.querySelector('.group-notice').textContent`;
  const readAgain=()=>click('.group-notice button','Reload board');
  async function menu(id,label,disabled=false) {
    const selector=section(id)+' .group-menu button';
    await click(selector,`Group menu for ${name(id)}`);
    const i=await observe(`[...document.querySelectorAll('${selector}')].findIndex(b=>!b.hidden && b.textContent===${JSON.stringify(label)})`);
    check(i>0,label);await click(selector+`:nth-child(${i+1})`,label,'button',1,0,disabled);
  }
  async function payload(rev,ids,collapsed) {
    check(browser.foldIds.length===1,'one presentation submission');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.foldIds.shift()});
    const d=JSON.parse(postData);
    check(Object.keys(d).length===3 && d.view_revision===rev && d.collapsed===collapsed && JSON.stringify(d.group_ids)===JSON.stringify(ids),'target IDs, view baseline and direction');
  }
  async function grid(folds=[],ids=ABC,checked=[],total=checked.length) {
    const expected=ids.map(id=>[id, name(id)+' / '+['No Items','1 item','2 items'][children[id].length],!folds.includes(id),folds.includes(id)?[]:children[id].map(n=>[uuid(n),checked.includes(n)])]);
    await wait(`JSON.stringify([...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,s.querySelector('h2').innerText,s.querySelector('.fold-toggle').getAttribute('aria-expanded')==='true',[...s.querySelectorAll('li')].map(r=>[r.dataset.itemId,r.querySelector('input').checked])]))===${JSON.stringify(JSON.stringify(expected))} && (document.querySelector('[role=toolbar] span')?.innerText ?? '')===${JSON.stringify(total?total+' selected':'')}`,'folded headings, complete counts, ordered visible selection');
    for(const id of ids) {
      const s=`document.querySelector('${section(id)}')`;
      check(await observe(folds.includes(id)?`!${s}.querySelector('input,.row-menu,.reorder-grip,.item-name,.add-item,ul')`:`!!${s}.querySelector(':scope > input[type=checkbox]') && !!${s}.querySelector('.add-item input') && ${s}.querySelectorAll('.row-menu').length===${children[id].length} && (${s}.querySelector('.add-item input').disabled || ${s}.querySelectorAll('.reorder-grip').length===${children[id].length})`),'group body availability');
    }
  }
  async function drop(source,target) {
    const point=async sel=>observe(`(()=>{const r=document.querySelector('${sel}').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2-2}})()`);
    const to=await point(section(target)+' h2');
    for (const [type,p] of [['mousePressed',await point(section(source)+' h2 > span')],['mouseMoved',to],['mouseReleased',to]]) await browser.send('Input.dispatchMouseEvent',{type,...p,button:'left',buttons:type==='mouseReleased'?0:1,clickCount:1});
  }
  switch(step) {
    case 'space': {
      await click(row(61),'Select Ink','checkbox');
      const focused=`document.activeElement.matches('${section(B)} .fold-toggle')`;
      for(let i=0;i<12 && !await observe(focused);i++) await key('Tab','Tab',9,8);
      check(await observe(focused),'keyboard reached B');await key(' ','Space',32,0,' ');
      await grid([B],ABC,[],1);exchange([post,200],[get,200]);await payload(1,[B],true);break;
    }
    case 'range':await click(row(59),'Select Ink','checkbox',1,8);await grid([B],ABC,[59],2);exchange([get,200]);break;
    case 'expand':await menu(B,'Expand group');await grid([],ABC,[59,61]);exchange([post,200],[get,200]);await payload(2,[B],false);break;
    case 'collapse':await menu(B,'Collapse group');await grid([B],ABC,[59],2);exchange([post,200],[get,200]);await payload(3,[B],true);break;
    case 'select hidden':await menu(B,'Select all items in group');await grid([B],ABC,[59],3);exchange([get,200]);await menu(C,'Select all items in group',true);await key('Escape','Escape',27);exchange();break;
    case 'reload B':await reload();await grid([B]);exchange([resolve,200],[get,200]);break;
    case 'move B':await menu(B,'Move group up');await grid([B],BAC);exchange([order,200],[get,200]);break;
    case 'member':await reload();await grid([B],BAC);check(await observe(`document.querySelector('#new-item-options button').disabled && !document.querySelector('.add-item input').disabled`),'Member access');exchange([resolve,200],[get,200]);break;
    case 'all':await menu(C,'Collapse all groups');await grid(BAC,BAC);exchange([post,200],[get,200]);await payload(4,BAC,true);break;
    case 'none':await menu(B,'Expand all groups');await grid([],BAC);exchange([post,200],[get,200]);await payload(5,BAC,false);break;
    case 'empty':await toggle(C);await grid([C],BAC);exchange([post,200],[get,200]);await payload(6,[C],true);await reload();await grid([C],BAC);exchange([resolve,200],[get,200]);break;
    case 'start B':case 'start C': {const id=step==='start B'?B:C;await toggle(id);await grid([id]);exchange([post,200],[get,200]);await payload(1,[id],true);break;}
    case 'drag B':await drop(B,A);await grid([B],BAC);exchange([order,200],[get,200]);break;
    case 'drag C':await drop(C,B);await grid([B],[C,B,A]);exchange([order,200],[get,200]);break;
    case 'after drag':await reload();await grid([B],[C,B,A]);exchange([resolve,200],[get,200]);await toggle(B);await grid([],[C,B,A]);exchange([post,200],[get,200]);await payload(2,[B],false);break;
    case 'viewer entry': {
      await browser.open(origin+'/boards/'+uuid(201));await identifierScreen();await passwordStep('guest');await type(password);await enter();await grid([C]);exchange([login,200],[resolve,200],[get,200]);
      await browser.send('Emulation.setDeviceMetricsOverride',{width:1100,height:1800,deviceScaleFactor:1,mobile:false});
      await toggle(C);await grid();await menu(C,'Collapse all groups');await grid(ABC);await menu(B,'Expand all groups');await grid();await menu(A,'Collapse all groups');await grid(ABC);await toggle(B);await click(row(61),'Select Ink','checkbox');await grid([A,C],ABC,[61]);exchange();
      await choose('other',origin+'/boards/'+uuid(201));await grid([C]);exchange([resolve,200],[get,200]);await choose('A');await grid([A,C],ABC,[61]);await reload();await grid([C]);exchange([resolve,200],[get,200]);
      await toggle(A);await grid([A,C]);await click('#back','Back to boards');await wait(`!!document.querySelector('[data-board-id="${uuid(202)}"]')`,'list');await click(`[data-board-id="${uuid(202)}"]`,'Review','link');await wait(`!!document.querySelector('[data-item-id="${uuid(62)}"]')`,'sibling');await click('#back','Back to boards');await wait(`!!document.querySelector('[data-board-id="${uuid(201)}"]')`,'list');await click(`[data-board-id="${uuid(201)}"]`,'Review','link');await grid([C]);exchange([list,200],[selectedGet(202),200],[list,200],[get,200]);break;
    }
    case 'pending':await toggle(B);await wait(`${notice}==='Saving main table…'`,'locked');await toggle(B);await grid();requests([post]);statuses();await payload(1,[B],true);break;
    case 'refused':await wait(`${notice}.startsWith('Main table change rejected.')`,'commit refused');await grid();requests([]);statuses([post,500]);break;
    case 'read baseline':await readAgain();await wait(`${notice}===''`,'fresh read');await grid();exchange([get,200]);break;
    case 'conflict':await toggle(B);await wait(`${notice}.startsWith('Main table changed.')`,'view conflict');await grid();exchange([post,409]);await payload(1,[B],true);break;
    case 'read peer':await readAgain();await grid([C]);exchange([get,200]);break;
    case 'fresh':await browser.failGroupRead(origin+'/preview/api'+get[1]);await toggle(B);await wait(`${notice}.startsWith('Could not read main table.')`,'post-commit read failed');await grid([C]);exchange([post,200],[get,503]);await payload(2,[B],true);break;
    case 'read commit':await browser.restoreGroupRead();await readAgain();await grid([B,C]);exchange([get,200]);break;
    case 'unconfirmed':await browser.failFoldConfirmation(origin+'/preview/api'+post[1]);await toggle(C);await wait(`${notice}.startsWith('Could not confirm main table change.')`,'unknown commit outcome');await grid([B,C]);exchange([post,503]);await payload(3,[C],false);break;
    case 'read unconfirmed':await browser.restoreGroupRead();await readAgain();await grid([B]);exchange([get,200]);break;
    case 'render member':await reload();await grid([B]);exchange([resolve,200],[get,200]);break;
    case 'denied':await toggle(B);await wait(`document.querySelector('main > [role=alert]').textContent==='Main table change denied.' && document.querySelector('.add-item input').disabled`,'demotion reread');await grid([B]);exchange([post,403],[get,200]);await payload(4,[B],false);break;
    case 'expired':await toggle(B);await identifierScreen();exchange([post,401]);await payload(4,[B],false);break;
    default:throw Error('unknown fold checkpoint');
  }
}
