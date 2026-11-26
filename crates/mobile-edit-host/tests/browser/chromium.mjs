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
  const traffic = [];
  const selectionStatuses = [];
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
      if (msg.sessionId === session && msg.method === 'Network.requestWillBeSent'
          && msg.params.type === 'Fetch') {
        const req = msg.params.request;
        traffic.push([req.method, req.url]); // no headers or bodies
      }
      if (msg.sessionId === session && msg.method === 'Network.responseReceived' && msg.params.type === 'Fetch') {
        const {url, status} = msg.params.response;
        if (new URL(url).pathname.startsWith('/navigation/boards/')) selectionStatuses.push(status);
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
      child.stdio[3].write(JSON.stringify({id, method, params, sessionId}) + '\0');
    });
  }
  return {
    windowClosed: Promise.race([closed, tabClosed]), traffic, selectionStatuses, send,
    async open(url) {
      const {targetInfos} = await send('Target.getTargets');
      const page = targetInfos.find(p => p.type === 'page' && p.url === 'about:blank');
      if (!page) throw Error('Missing owned tab');
      pageId = page.targetId;
      if (foreground) await send('Target.setDiscoverTargets', {discover: true});
      ({sessionId: session} = await send('Target.attachToTarget', {targetId: page.targetId, flatten: true}));
      await send('Network.enable', {maxPostDataSize: 0});
      await send('Page.navigate', {url});
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
