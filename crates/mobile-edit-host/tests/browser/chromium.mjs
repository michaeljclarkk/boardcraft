// CDP uses inherited pipes 3/4, never a listening debugger or an existing profile.
import {spawn} from 'node:child_process';
import {existsSync} from 'node:fs';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';

function executable() {
  if (process.env.BOARDCRAFT_CHROMIUM) return process.env.BOARDCRAFT_CHROMIUM;
  for (const root of [process.env['ProgramFiles(x86)'], process.env.ProgramFiles].filter(Boolean)) {
    for (const path of ['Microsoft/Edge/Application/msedge.exe', 'Google/Chrome/Application/chrome.exe']) {
      if (existsSync(join(root, path))) return join(root, path);
    }
  }
  return 'chromium';
}

export function launch(profile, foreground) {
  const child = spawn(executable(), [
    `--user-data-dir=${profile}`, '--remote-debugging-pipe', '--no-first-run',
    '--no-default-browser-check', '--disable-extensions', '--disable-background-networking',
    '--disable-sync', '--disable-component-update', '--disable-breakpad', '--disable-crash-reporter',
    // An occluded startup window can leave the fixture hidden and suppress HTML autofocus.
    '--inprivate', '--incognito', foreground ? '--disable-backgrounding-occluded-windows' : '--headless=new',
    'about:blank'
  ], {stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'], windowsHide: !foreground});
  const pending = new Map();
  const tabs = new Map(), logs = new Map();
  let context;
  let nameCase = false;
  let seq = 0, session, buffer = '', ended = false, pageId, windowClosed;
  const tabClosed = new Promise(resolve => { windowClosed = resolve; });
  const closed = new Promise(resolve => {
    child.once('error', () => {}); // 'close' also follows spawn failure
    child.once('close', () => {
      ended = true;
      for (const done of pending.values()) done();
      resolve();
    });
  });
  child.stdio[3].on('error', () => {});
  child.stdio[4].setEncoding('utf8').on('data', chunk => {
    buffer += chunk;
    let end;
    while ((end = buffer.indexOf('\0')) >= 0) {
      const msg = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      if (msg.id) pending.get(msg.id)?.(msg);
      if (msg.method === 'Target.targetDestroyed' && msg.params.targetId === pageId) windowClosed();
      const tab = logs.get(msg.sessionId);
      if (tab?.probe && msg.method === 'Network.requestWillBeSentExtraInfo') {
        const headers = msg.params.headers;
        const get = name => Object.entries(headers).find(([k])=>k.toLowerCase()===name)?.[1];
        const uuid = '[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}';
        const inert = !get('authorization') && !get('boardcraft-preview') &&
          new RegExp(`^boardcraft-${uuid}=${uuid}$`).test(get('cookie') ?? '');
        if (tab.probe.extra.size < 4) tab.probe.extra.set(msg.params.requestId,inert);
      }
      if (tab?.probe && msg.method === 'Network.responseReceived' && msg.params.type === 'Document' &&
          new URL(msg.params.response.url).pathname === '/preview/api/session') {
        tab.probe.denied = [401,403].includes(msg.params.response.status);
        tab.probe.id = msg.params.requestId;
      }
      if (tab && msg.method === 'Network.requestWillBeSent'
          && msg.params.type === 'Fetch') {
        const req = msg.params.request;
        tab.traffic.push([req.method, req.url]); // no headers or bodies
        if (req.method === 'POST' && new URL(req.url).pathname ===
            '/preview/api/navigation/boards/00000000-0000-0000-0000-0000000000c9/items') {
          tab.createIds.push(msg.params.requestId);
        }
        if (nameCase && req.method === 'PUT' && new URL(req.url).pathname ===
            '/preview/api/navigation/boards/00000000-0000-0000-0000-0000000000c9/items/00000000-0000-0000-0000-00000000003d/name') {
          tab.renameIds.push(msg.params.requestId);
        }
      }
      if (tab && msg.method === 'Network.responseReceived' && msg.params.type === 'Fetch') {
        const {url, status} = msg.params.response;
        tab.responses.push([new URL(url).pathname, status, msg.params.response.headers['boardcraft-board-access'] ?? null]);
        if (new URL(url).pathname.startsWith('/preview/api/navigation/boards/')) tab.selectionStatuses.push(status);
      }
    }
  });
  function send(method, params = {}, sessionId = session) {
    return new Promise((resolve, reject) => {
      if (ended) return reject(Error('Browser closed'));
      const id = ++seq;
      const done = msg => {
        clearTimeout(timer); pending.delete(id);
        if (msg?.result) resolve(msg.result); else reject(Error(`Browser command failed: ${method}`));
      };
      const timer = setTimeout(done, 8000);
      pending.set(id, done);
      child.stdio[3].write(JSON.stringify({id, method, params, sessionId:sessionId ?? undefined}) + '\0');
    });
  }
  async function attach(name, targetId, url) {
    ({sessionId: session} = await send('Target.attachToTarget', {targetId, flatten:true}, null));
    tabs.set(name, session);
    logs.set(session, {traffic:[], responses:[], selectionStatuses:[], renameIds:[], createIds:[], probe:name === 'peer' && {extra:new Map()}});
    await send('Network.enable', {maxPostDataSize:0});
    await send('Page.navigate', {url});
  }
  return {
    windowClosed: Promise.race([closed, tabClosed]), send,
    get traffic() { return logs.get(session).traffic; },
    get selectionStatuses() { return logs.get(session).selectionStatuses; },
    get responses() { return logs.get(session).responses; },
    get renameIds() { return logs.get(session).renameIds; },
    get createIds() { return logs.get(session).createIds; },
    async isolatedProbe() {
      const tab = logs.get(session), p = tab.probe;
      for (let n = 0; n < 80 && !(p.id && p.extra.has(p.id)); n++) await delay(25);
      tab.probe = null;
      return p.denied && p.extra.get(p.id);
    },
    select(name) { session = tabs.get(name); if (!session) throw Error('missing tab'); },
    async newTab(name,url) {
      const {targetId} = await send('Target.createTarget', {url:'about:blank',browserContextId:context,background:name !== 'A'}, null);
      await attach(name,targetId,url);
    },
    async open(url, names = false) {
      nameCase = names;
      const {targetInfos} = await send('Target.getTargets');
      const page = targetInfos.find(p => p.type === 'page' && p.url === 'about:blank');
      if (!page) throw Error('Missing owned tab');
      pageId = page.targetId;
      if (foreground) await send('Target.setDiscoverTargets', {discover: true});
      if (foreground) await attach('A',pageId,url);
      else {
        ({browserContextId:context} = await send('Target.createBrowserContext'));
        await this.newTab('A',url);
      }
    },
    async close() {
      if (!ended) {
        child.stdio[3].write(JSON.stringify({id: ++seq, method: 'Browser.close'}) + '\0');
        await Promise.race([closed, delay(4000, null, {ref: false})]);
        if (!ended) child.kill();
      }
      await closed;
    }
  };
}
