const groups = [
  ['g-90','Work','4 items',[[73,'Sketch'],[61,'Ink'],[67,'Ink'],[77,'Fold']]],
  ['g-10','Reserve','No Items',[]],
  ['g-20','Work','1 item',[[59,'Ink']]]
];
let viewer;

export async function selectionJourney(action, {uuid, send, click, key, wait, requests, statuses,
  reload, historyAt, selectedGet, boardPath, resolve, observe, named}) {
  const get = selectedGet(201);
  const section = id => `#items > section[data-group-id="${id}"]`;
  const row = n => `${section(groups.find(g => g[3].some(([id]) => id===n))[0])} li[data-item-id="${uuid(n)}"]`;
  const box = 'input[type=checkbox]';
  const tools = '[role=toolbar][aria-label="Selected item actions"]';
  const header = id => {
    const name = `Select all items in ${groups.find(g => g[0]===id)[1]}`;
    return click(`${section(id)} > ${box}[aria-label="${name}"]`,name,'checkbox',1,0,id==='g-10');
  };
  const toggle = (n,shift=false) => {
    const name = 'Select '+groups.flatMap(g=>g[3]).find(([id])=>id===n)[1];
    return click(`${row(n)} ${box}[aria-label="${name}"]`,name,'checkbox',1,shift ? 8 : 0);
  };
  const clear = () => click(tools+' button','Clear item selection');
  async function state(ids, exchanges = [], movement = 'stay') {
    const expected = groups.map(([id,name,heading,items]) => {
      const n = items.filter(([i]) => ids.includes(i)).length;
      return [id,`${name} / ${heading}`, n>0 && n===items.length, n>0 && n<items.length, !items.length,
        items.map(([i,title]) => [uuid(i),`Select ${title}`,ids.includes(i),ids.includes(i)])];
    });
    await wait(`(() => {
      const actual = Array.from(document.querySelectorAll('#items > section'), s => {
        const h = s.querySelector(':scope > ${box}');
        return [s.dataset.groupId,s.querySelector('h2').innerText,h.checked,h.indeterminate,h.disabled,
          Array.from(s.querySelectorAll('li'),r => { const b=r.querySelector('${box}:enabled');
            return [r.dataset.itemId,b.getAttribute('aria-label'),b.checked,getComputedStyle(r).backgroundColor !== 'rgba(0, 0, 0, 0)']; })];
      });
      const bar=document.querySelector('${tools}');
      return JSON.stringify(actual)===${JSON.stringify(JSON.stringify(expected))} &&
        document.querySelectorAll('#destination ${box}').length===8 &&
        (${ids.length} ? bar?.getBoundingClientRect().height>0 && bar.querySelector('span').innerText==='${ids.length} selected' : !bar) &&
        document.querySelectorAll('#items button.item-name').length===${viewer?0:5} &&
        document.querySelectorAll('#items .reorder-grip').length===${viewer?0:5} &&
        ${viewer ? "!document.querySelector('.row-menu button:last-child:enabled,.add-item :enabled') &&" : ''}
        !document.querySelector('#items li input:not([type=checkbox]),dialog,[aria-expanded=true],[data-drop-edge]');
    })()`, 'selection marks counts and controls');
    if (ids.length) { await named('toolbar','Selected item actions'); await named('button','Clear item selection'); }
    requests(exchanges.map(([req]) => req)); statuses(...exchanges);
    await historyAt(boardPath(201),movement);
  }
  async function guardedDrag() {
    for (const [type,selector] of [['mousePressed',row(67)+' .reorder-grip'],['mouseMoved',row(61)],['mouseReleased',row(61)]]) {
      const pt = await observe(`(() => { const e=document.querySelector('${selector}'),r=e.getBoundingClientRect();
        const x=r.x+r.width/2,y=r.y+r.height/4; return e.contains(document.elementFromPoint(x,y)) && {x,y}; })()`);
      if (!pt) throw Error('drag target');
      await send('Input.dispatchMouseEvent',{type,...pt,button:'left',buttons:type==='mouseReleased'?0:1,clickCount:1});
      if (type==='mouseMoved') await wait(`!document.querySelector('[data-drop-edge]')`,'multi drag guarded');
    }
    await state([67,59]);
  }
  switch (action) {
    case 'select member': case 'select viewer':
      viewer = action==='select viewer';
      statuses([['POST','/session'],200],[resolve,200],[['GET','/navigation/boards'],200]);
      await send('Emulation.setDeviceMetricsOverride',{width:1000,height:1150,deviceScaleFactor:1,mobile:false});
      await click(`[data-board-id="${uuid(201)}"]`,'Review','link');
      await state([],[[get,200]],'push'); break;
    case 'select duplicates': case 'select reverse':
      await toggle(67); await state([67]);
      await key(' ','Space',32,0,' '); await state([]);
      await key(' ','Space',32,0,' '); await state([67]);
      await toggle(59); await state([67,59]);
      if (!viewer) await guardedDrag();
      await header('g-90'); await state([73,61,67,77,59],[[get,200]]);
      await header('g-90'); await state([59]);
      await header('g-10'); await state([59]);
      await clear(); await state([]);
      if (action==='select reverse') {
        await toggle(67,true); await state([67,77,59]);
        await toggle(77,true); await state([67]);
      }
      break;
    case 'select reload':
      await reload(); await state([],[[resolve,200],[get,200]]);
      await toggle(73,true); await state([73]); await toggle(67); await state([73,67]); break;
    case 'select failed':
      await toggle(59,true);
      await wait(`document.querySelector('#failures').innerText.includes('Could not resolve selection')`,'selection read failure');
      await state([73,67],[[get,503]]); break;
    case 'select retry':
      await toggle(59,true); await state([73,67,77,59],[[get,200]]);
      await toggle(59,true); await state([73]);
      await clear(); await state([]); break;
    default: throw Error('unknown selection checkpoint');
  }
}
