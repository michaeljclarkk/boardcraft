import {setTimeout as delay} from 'node:timers/promises';

const A='g-old', B='g-z', C='g-end', ABC=[A,B,C], CAB=[C,A,B], BAC=[B,A,C], BCA=[B,C,A];
const section = id => `#items > section[data-group-id="${id}"]`;
const title = id => section(id)+' h2 > span';
const table = `document.querySelector('#items')`;
const cue = `document.querySelector('body > [role=status]')`;
const marked = `document.querySelector('#items h2[data-drop-edge]')`;

export async function headingDragJourney(step, {browser,uuid,click,key,wait,check,requests,statuses,reload,resolve,observe,list,origin,grid,payload,exchange,post,get,notice}) {
  async function point(selector,dy=0) {
    const p = await observe(`(() => {const el=document.querySelector(${JSON.stringify(selector)}), r=el.getBoundingClientRect();
      const x=r.left+r.width/2,y=r.top+r.height/2+${dy}; return el.contains(document.elementFromPoint(x,y)) && {x,y};})()`);
    check(p,'visible pointer target '+selector); return p;
  }
  const mouse = (type,p,button='left',count=1) => browser.send('Input.dispatchMouseEvent',{
    type,...p,button,buttons:type==='mouseReleased'?0:button==='right'?2:1,clickCount:count
  });
  const press = (p,b,n) => mouse('mousePressed',p,b,n);
  const travel = (p,b) => mouse('mouseMoved',p,b);
  const release = (p,b,n) => mouse('mouseReleased',p,b,n);
  const esc = () => key('Escape','Escape',27);
  const clean = () => wait(`!${cue} && !${marked} && !document.querySelector('dialog[open], #items input:checked, #items [aria-expanded=true]:not(.fold-toggle)')`,'clean table after drag');
  const label = id => wait(`${cue}?.innerText==='${id===C?'Reserve':'Work'}' && ${cue}.getBoundingClientRect().width>0`, 'moving group name');
  const mark = (id,side) => wait(`${marked}?.parentNode.dataset.groupId==='${id}' && ${marked}.dataset.dropEdge==='${side}' && getComputedStyle(${marked}).boxShadow!=='none'`, 'visible '+side+' mark on '+id);
  async function hold(source=C,target=A,dy=-2) {
    await press(await point(title(source)));
    const dest=await point(section(target)+' h2',dy); await travel(dest);
    await label(source); await mark(target,dy<=0?'before':'after'); return dest;
  }
  const resize = height => browser.send('Emulation.setDeviceMetricsOverride',{width:1100,height,deviceScaleFactor:1,mobile:false});
  async function inView(selector) {
    const {root} = await browser.send('DOM.getDocument');
    const {nodeId} = await browser.send('DOM.querySelector',{nodeId:root.nodeId,selector});
    await browser.send('DOM.scrollIntoViewIfNeeded',{nodeId});
  }
  const viewport = () => observe(`(() => {const r=${table}.getBoundingClientRect(); return {x:r.left+r.width/2,top:Math.max(r.top,0),bottom:Math.min(r.bottom,innerHeight)};})()`);
  const scroll = () => observe(`${table}.scrollTop`);
  switch(step) {
    case 'empty target': {
      const p=await hold(B,C,0); await travel({...p,y:p.y+1}); await mark(C,'after'); await esc();
      await release(p); await clean(); await grid(ABC); exchange(); break;
    }
    case 'pending': {
      await release(await hold()); await wait(`${notice}==='Saving group order…'`,'board lock holds drop');
      const p=await point(title(C)), dest=await point(section(A)+' h2');
      await press(p); await travel(dest); await release(dest); await clean(); await grid(ABC);
      requests([post]); statuses(); await payload(7,CAB); break;
    }
    case 'committed': await grid(CAB); requests([get]); statuses([post,200],[get,200]); break;
    case 'child': {
      await press(await point(title(C))); const row=await point(section(B)+' li:last-child .item-name');
      await travel(row); await mark(B,'after'); await release(row); await grid(ABC); await clean();
      exchange([post,200],[get,200]); await payload(8,ABC); break;
    }
    case 'reopen': await reload(); await grid(ABC); exchange([resolve,200],[get,200]); break;
    case 'cancel': {
      const p=await point(title(C)), dest=await point(section(A)+' h2',-3);
      await press(p); await esc(); await travel(dest); await clean(); await release(dest);
      await press(p); await travel({...p,x:p.x+3,y:p.y+4}); await clean();
      await travel(p); await clean(); await travel({...p,x:p.x+6}); await label(C);
      await travel(dest); await mark(A,'before'); await esc(); await clean(); await release(dest);

      // Observe whether the native release click reaches the bubble phase.
      await observe(`window.tableClicks=0; ${table}.addEventListener('click',()=>window.tableClicks++); true`);
      const b=await point(title(B)); await press(b); await travel(dest); await mark(A,'before');
      await esc(); await release(b); check(await observe('window.tableClicks')===0,'cancelled release click consumed');
      await click(title(B),null,null); check(await observe('window.tableClicks')===1,'next deliberate click delivered');
      await click(title(B),null,null,2); await wait(`!!document.querySelector('dialog[open] input')`,'ordinary double-click opens editor'); await esc();
      await release(await hold(B,A,1)); await clean(); // B already follows A
      await press(p); const self={...p,x:p.x+8}; await travel(self); await label(C); await release(self); await clean();
      await hold(); const outside=await point('#back'); await travel(outside); await release(outside); await clean();
      await hold();
      // CDP has no mouse-cancel command. Cancel the captured pointer stream
      // after native activation, then finish with a real release at the target.
      check(await observe(`document.querySelector('${title(C)}').dispatchEvent(new PointerEvent('pointercancel',{bubbles:true,pointerId:1}))`),'pointer cancellation');
      await clean(); await release(dest); await grid(ABC); exchange(); break;
    }
    case 'wrong surfaces': {
      const dest=await point(section(C)+' h2');
      for (const selector of ['h2','.group-menu button','li .item-name','li .reorder-grip','li input[type=checkbox]']) {
        await press(await point(section(B)+' '+selector)); await travel(dest); await clean(); await release(dest);
      }
      // Hit the text node containing the count, not the adjacent name span.
      const count=await observe(`(() => {const range=new Range();range.selectNodeContents(document.querySelector('${section(B)} h2').lastChild);const r=range.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`);
      await press(count); await travel(dest); await clean(); await release(dest);
      const p=await point(title(A)); await press(p,'right'); await travel(dest,'right'); await clean(); await release(dest,'right'); await esc();
      await browser.send('Emulation.setTouchEmulationEnabled',{enabled:true});
      for(const [type,touchPoints] of [['touchStart',[p]],['touchMove',[dest]],['touchCancel',[]]]) {
        await browser.send('Input.dispatchTouchEvent',{type,touchPoints}); await clean();
      }
      await browser.send('Emulation.setTouchEmulationEnabled',{enabled:false}); await grid(ABC); exchange(); break;
    }
    case 'scroll': {
      await resize(440); await inView('#items');
      const start=await scroll(), v=await viewport(), edge={x:v.x,y:v.bottom-5};
      check(await observe(`document.elementFromPoint(${edge.x},${edge.y}).closest('section').dataset.groupId`)===B,'B under pointer before scrolling');
      await press(await point(title(A))); await travel(edge); await label(A);
      await wait(`${table}.scrollTop>${start}`,'still pointer scrolled'); await mark(C,'after');
      await release(edge); await grid(BCA); await clean(); exchange([post,200],[get,200]); await payload(7,BCA); break;
    }
    case 'stop scrolling': {
      await inView(title(A)); const v=await viewport(), edge={x:v.x,y:v.top+5}, middle={x:v.x,y:(v.top+v.bottom)/2};
      const start=await scroll(); await press(await point(title(A))); await travel(edge);
      await wait(`${table}.scrollTop<${start}`,'scroll toward top'); await travel(middle);
      const stopped=await scroll(); await delay(100); check(stopped===await scroll(),'stops away from edge');
      await travel(edge); await esc(); await release(edge); const cancelled=await scroll(); await delay(100);
      check(cancelled===await scroll(),'Escape stops scrolling'); await clean(); await grid(BCA); exchange(); break;
    }
    case 'leave': {
      await resize(1800); const p=await hold(C,B); const history=await browser.send('Page.getNavigationHistory');
      await browser.send('Page.navigateToHistoryEntry',{entryId:history.entries[history.currentIndex-1].id});
      await wait(`!!document.querySelector('[data-board-id="${uuid(201)}"]')`,'left mounted visit'); await release(p);
      await click(`[data-board-id="${uuid(201)}"]`,'Review','link'); await clean(); await grid(BCA); exchange([list,200],[get,200]); break;
    }
    case 'refused': await release(await hold()); await wait(`${notice}.startsWith('Could not move group.')`,'commit refused'); await clean(); await grid(ABC); exchange([post,500]); await payload(7,CAB); break;
    case 'hold': await hold(); await grid(ABC); exchange(); break;
    case 'stale': await release(await point(section(A)+' h2',-2)); await wait(`${notice}==='Group order changed. Reload board'`,'stale drop'); await grid(ABC); exchange([post,409]); await payload(7,CAB); break;
    case 'read peer': case 'read commit':
      if(step==='read commit') await browser.restoreGroupRead();
      await click('.group-notice button','Reload board'); await grid(step==='read peer'?BAC:BCA); exchange([get,200]); break;
    case 'fresh': await browser.failGroupRead(origin+'/preview/api'+get[1]); await release(await hold()); await wait(`${notice}==='Could not read group order. Reload board'`,'refresh failed'); await grid(BAC); exchange([post,200],[get,503]); await payload(8,BCA); break;
    case 'denied': await release(await point(section(A)+' h2',-2)); await wait(`document.querySelector('main > [role=alert]').innerText==='Group reorder denied.' && document.querySelector('#new-item-options button').disabled`,'Owner rechecked'); await grid(ABC); exchange([post,403],[get,200]); await payload(7,CAB); break;
    case 'member': case 'viewer': {
      if(step==='viewer') {await reload(); await grid(ABC); exchange([resolve,200],[get,200]);}
      const p=await point(section(A)+' h2'); await press(await point(title(C))); await travel(p); await clean(); await release(p); await grid(ABC); exchange(); break;
    }
    default: throw Error('unknown drag checkpoint');
  }
}
