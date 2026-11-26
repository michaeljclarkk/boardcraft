export async function groupOrderJourney(step, {browser,uuid,click,key,wait,check,requests,statuses,reload,selectedGet,resolve,observe,list,origin}) {
  const get = selectedGet(201), post = ['POST',get[1]+'/group-order'];
  const A='g-old', B='g-z', C='g-end', ABC=[A,B,C], BAC=[B,A,C], ACB=[A,C,B];
  const exchange = (...pairs) => { requests(pairs.map(([r])=>r)); statuses(...pairs); };
  const section = id => `section[data-group-id="${id}"] .group-menu`;
  const menu = (id=B) => click(section(id)+' button:first-child',`Group menu for ${id===C?'Reserve':'Work'}`);
  const choice = (direction,id=B,disabled=false) => click(section(id)+` button:nth-child(${direction==='up'?4:5})`,`Move group ${direction}`,'button',1,0,disabled);
  const dismiss = () => key('Escape','Escape',27);
  const notice = `document.querySelector('.group-notice').textContent`;
  async function grid(order) {
    const heading = { [A]:'Work / 1 item', [B]:'Work / 2 items', [C]:'Reserve / No Items', 'g-peer':'Work / 1 item' };
    const children = { [A]:[59], [B]:[61,63], [C]:[], 'g-peer':[62] };
    const expected = order.map(id=>[id,heading[id],children[id].map(n=>[uuid(n),'Ink'])]);
    await wait(`JSON.stringify([...document.querySelectorAll('#items section')].map(s=>[s.dataset.groupId,s.querySelector('h2').innerText,[...s.querySelectorAll('li')].map(r=>[r.dataset.itemId,r.querySelector('.item-name').innerText])]))===${JSON.stringify(JSON.stringify(expected))}`, 'group IDs, counts and ordered Ink rows');
  }
  async function payload(rev,ids) {
    check(browser.groupOrderIds.length===1,'one group-order submission');
    const {postData}=await browser.send('Network.getRequestPostData',{requestId:browser.groupOrderIds.shift()});
    const d=JSON.parse(postData);
    check(d.board_revision===rev && Object.keys(d).length===2 && JSON.stringify(d.group_ids)===JSON.stringify(ids),'complete requested permutation and revision');
  }
  async function accessible(id, up, down) {
    await menu(id);
    const states=await observe(`[...document.querySelectorAll('${section(id)} button')].filter(b=>b.textContent.startsWith('Move group') && !b.hidden).map(b=>[b.textContent,!b.disabled])`);
    check(JSON.stringify(states)===JSON.stringify([['Move group up',up],['Move group down',down]]),'both direction permissions');
  }
  switch(step) {
    case 'boundaries':
      await accessible(A,false,true); await choice('up',A,true); await dismiss();
      await accessible(C,true,false); await choice('down',C,true); await dismiss();
      await accessible(B,true,true); await dismiss(); await grid(ABC); exchange(); break;
    case 'open': await accessible(B,true,true); exchange(); break;
    case 'pending':
      await menu(); await choice('up'); await wait(`${notice}==='Saving group order…'`,'waiting for lock');
      await menu(); await wait(`document.querySelector('${section(B)} button').getAttribute('aria-expanded')==='false'`,'menu stays closed');
      await grid(ABC); requests([post]); statuses(); await payload(7,BAC); break;
    case 'committed': await grid(BAC); requests([get]); statuses([post,200],[get,200]); break;
    case 'up': case 'down': case 'past empty': {
      const [direction,rev,order] = step==='up'?['up',7,BAC]: step==='down'?['down',8,ABC]:['down',9,ACB];
      await menu(); await choice(direction); await grid(order); exchange([post,200],[get,200]); await payload(rev,order); break;
    }
    case 'document reload': await reload(); await grid(ACB); exchange([resolve,200],[get,200]); break;
    case 'sole':
      await click('#back','Back to boards'); await wait(`!!document.querySelector('[data-board-id="${uuid(202)}"]')`,'board list');
      await click(`[data-board-id="${uuid(202)}"]`,'Review','link'); await grid(['g-peer']);
      await accessible('g-peer',false,false); await choice('up','g-peer',true); await choice('down','g-peer',true); exchange([list,200],[selectedGet(202),200]); break;
    case 'refusal': await menu(); await choice('down'); await wait(`${notice}.startsWith('Could not move group.')`,'commit refused'); await grid(BAC); exchange([post,500]); await payload(8,ABC); break;
    case 'conflict': await choice('up'); await wait(`${notice}==='Group order changed. Reload board'`,'stale menu conflict'); await grid(ABC); exchange([post,409]); await payload(9,BAC); break;
    case 'read peer':case 'read commit':
      if(step==='read commit') await browser.restoreGroupRead();
      await click('.group-notice button','Reload board'); await grid(step==='read peer'?ACB:ABC); exchange([get,200]); break;
    case 'new neighbour':
      await browser.failGroupRead(origin+'/preview/api'+get[1]); await menu(); await choice('up');
      await wait(`${notice}==='Could not read group order. Reload board'`,'accepted order needs refresh'); await grid(ACB); exchange([post,200],[get,503]); await payload(10,ABC); break;
    case 'denied': await choice('up'); await wait(`document.querySelector('main > [role=alert]').textContent==='Group reorder denied.' && document.querySelector('#new-item-options button').disabled`,'reread current access'); await grid(ABC); exchange([post,403],[get,200]); await payload(7,BAC); break;
    case 'member':case 'viewer':
      if(step==='viewer'){await reload(); await grid(ABC); exchange([resolve,200],[get,200]);}
      await accessible(B,false,false); await choice('up',B,true); await choice('down',B,true);
      check(await observe(`!!document.querySelector('button.item-name:enabled')`)===(step==='member'),'item edit permission'); exchange(); break;
    default: throw Error('unknown group order checkpoint');
  }
}
