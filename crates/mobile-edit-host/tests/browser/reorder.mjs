let releaseAt;
export async function reorderJourney(action, {uuid, send, click, key, wait, requests, statuses, reload, historyAt, selectedGet, boardPath, resolve, observe}) {
  const get = selectedGet(201), post = ['POST',get[1]+`/items/${uuid(67)}/reorder`];
  const row = n => `[data-item-id="${uuid(n)}"]`;
  async function point(selector, fraction=.5) {
    return observe(`(() => { const e=document.querySelector('${selector}'),r=e.getBoundingClientRect();
      const x=r.x+r.width/2,y=r.y+r.height*${fraction}; return e.contains(document.elementFromPoint(x,y)) && {x,y}; })()`);
  }
  const mouse = (type,pt) => send('Input.dispatchMouseEvent',{type,...pt,button:'left',buttons:type==='mouseReleased'?0:1,clickCount:1});
  async function press() { const pt=await point(row(67)+' .reorder-grip'); await mouse('mousePressed',pt); return pt; }
  async function mark(side) {
    await press(); releaseAt=await point(row(61),side==='before'?.25:.75);
    await mouse('mouseMoved',releaseAt);
    await wait(`document.querySelector('${row(61)}')?.dataset.dropEdge==='${side}' && getComputedStyle(document.querySelector('${row(61)}')).boxShadow !== 'none'`, 'visible drop edge');
  }
  const release=()=>mouse('mouseReleased',releaseAt);
  async function grid(before=false,viewer=false) {
    const ids=(before?[73,67,61,77,59]:[73,61,67,77,59]).map(uuid);
    await wait(`(() => { const rows=[...document.querySelectorAll('#items li')];
      return rows.map(r=>r.dataset.itemId).join()==='${ids}' && rows.map(r=>r.querySelector('.item-name')?.innerText).join()==='Sketch,Ink,Ink,Fold,Ink' &&
        [...document.querySelectorAll('#items section')].map(s=>s.dataset.groupId+':'+s.querySelector('h2').innerText).join()==='g-90:Work / 4 items,g-10:Reserve / No Items,g-20:Work / 1 item' &&
        document.querySelectorAll('.reorder-grip').length===${viewer?0:5} && !document.querySelector('#items li input,dialog,[aria-expanded=true]'); })()`, 'accepted rows, counts and controls');
  }
  let movement='stay';
  switch(action) {
    case 'reorder open':
      statuses([['POST','/session'],200],[resolve,200],[['GET','/navigation/boards'],200]);
      await send('Emulation.setDeviceMetricsOverride',{width:1000,height:1100,deviceScaleFactor:1,mobile:false});
      await click('#boards li:last-child a','Review','link'); await grid(); requests([get]); statuses([get,200]); movement='push'; break;
    case 'reorder cancel': {
      const pt=await press(), near={x:pt.x+5,y:pt.y}; await mouse('mouseMoved',near); await mouse('mouseReleased',near);
      await mark('before'); await key('Escape','Escape',27); await wait(`!document.querySelector('[data-drop-edge]')`,'cancelled marker'); await release();
      for(const selector of [row(67),row(59),'#items h2']) {await press(); const p=await point(selector); await mouse('mouseMoved',p); await mouse('mouseReleased',p);}
      await grid(); requests([]); statuses(); break;
    }
    case 'reorder mark': await mark('before'); await grid(); requests([]); statuses(); break;
    case 'reorder pending':
      await release(); await wait(`document.querySelector('#failures').innerText.includes('Saving order…')`,'pending');
      await press(); await mouse('mouseMoved',releaseAt); await release(); await grid(); requests([post]); statuses(); break;
    case 'reorder committed': await grid(true); requests([get]); statuses([post,200],[get,200]); break;
    case 'reorder before': case 'reorder after': {
      const before=action==='reorder before'; await mark(before?'before':'after'); await release(); await grid(before); requests([post,get]); statuses([post,200],[get,200]); break;
    }
    case 'reorder denied': await release(); await grid(false,true); await wait(`document.querySelector('main > [role=alert]').innerText==='Item reorder denied.'`,'denial'); requests([post,get]); statuses([post,403],[get,200]); break;
    case 'reorder failed': await release(); await wait(`document.querySelector('#failures').innerText.includes('Could not confirm order.') && !document.querySelector('[data-drop-edge],dialog') && document.querySelector('#failures button').innerText==='Reload board'`, 'table failure'); await grid(); requests([post]); statuses([post,500]); break;
    case 'reorder viewer reload': case 'reorder initial reload': case 'reorder before reload': case 'reorder after reload': {
      const viewer=action==='reorder viewer reload'; await reload(); await grid(action==='reorder before reload', viewer);
      if(viewer) { await mouse('mousePressed',await point(row(67))); await mouse('mouseMoved',await point(row(61))); await mouse('mouseReleased',await point(row(61))); }
      requests([resolve,get]); statuses([resolve,200],[get,200]); break;
    }
    default: throw Error('unknown reorder checkpoint');
  }
  await historyAt(boardPath(201),movement);
}
