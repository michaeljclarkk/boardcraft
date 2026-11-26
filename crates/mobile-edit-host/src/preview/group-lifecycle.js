// A removal message belongs to this mounted visit only. Its label is never a
// restore target. Both Restore gestures submit the same board-scoped command.
export function groupMenus({api, board, busy, setBusy, refresh, button}, notice, edit) {
  async function submit(group, action) {
    if (busy() || !api.current() || !board().owner) return;
    setBusy(true); notice.replaceChildren();
    try {
      await api.transitionGroup(group.id, action, board().revision);
    } catch (error) {
      if (!api.current()) return;
      setBusy(false);
      if (error === 401) api.expired();
      else if (error === 403) api.denied('Group change denied.');
      else if (error === 500) notice.textContent = 'Could not change group. Reopen its menu or Archive to retry.';
      else api.reload(error === 409 ? 'Board changed. Reload the board.' : 'Group outcome unavailable. Reload the board.');
      return;
    }
    if (!api.current()) return;
    try {
      const latest = await api.read();
      if (!api.current()) return;
      await api.readArchive();
      if (!api.current()) return;
      setBusy(false); refresh(latest);
      if (action === 'archive' && latest.owner && !latest.groups.some(g => g.id === group.id)) {
        notice.append(`${group.name} was removed. `, button('Undo', () => submit(group, 'restore')),
          button('Dismiss', () => notice.replaceChildren()));
      }
    } catch (error) {
      if (!api.current()) return;
      if (error === 401) api.expired();
      else if (error === 403) api.denied('Group changed.');
      else api.reload('Group changed. Reload the board.');
    }
  }
  return (group, index) => {
    const menu = document.createElement('span'); menu.className = 'group-menu';
    const toggle = button('⋯', () => {
      if (busy()) return;
      expand(archive.hidden);
      if (!archive.hidden && !archive.disabled) archive.focus();
    });
    toggle.setAttribute('aria-label', `Group menu for ${group.name}`);
    const archive = button('Archive group', () => { expand(false); submit(group, 'archive'); });
    const actions = ['Rename group','Change group colour'].map(label => button(label, () => { expand(false); edit(group, toggle); }));
    for (const [label, delta] of [['Move group up', -1], ['Move group down', 1]]) {
      const neighbour = index + delta;
      const action = button(label, () => {
        if (action.disabled || busy() || !api.current()) return;
        expand(false); toggle.focus();
        const ids = board().groups.map(g => g.id);
        [ids[index], ids[neighbour]] = [ids[neighbour], ids[index]];
        move(ids);
      });
      action.disabled = neighbour < 0 || neighbour >= board().groups.length;
      actions.push(action);
    }
    actions.push(archive);
    for (const action of actions) {
      if (!board().owner) { action.disabled = true; action.title = 'Owner access required.'; }
    }
    function expand(open) { for (const action of actions) action.hidden = !open; toggle.setAttribute('aria-expanded', String(open)); }
    menu.onkeydown = e => { if (e.key === 'Escape') { expand(false); toggle.focus(); } };
    menu.onfocusout = e => { if (!menu.contains(e.relatedTarget)) expand(false); };
    expand(false); menu.append(toggle, ...actions); return menu;
  };

  // Until an authoritative read succeeds, keep the previous grid and block
  // all new choices. Recovery never retains the submitted IDs or direction.
  function reload(message) {
    notice.replaceChildren(message+' ', button('Reload board', readOrder));
  }
  function accessChanged(error) {
    if (error === 401) api.expired();
    else if (error === 403) api.denied('Group reorder denied.');
    else return false;
    return true;
  }
  async function readOrder() {
    if (!api.current()) return;
    notice.textContent = 'Reading board…';
    try {
      const latest = await api.read();
      if (!api.current()) return;
      setBusy(false); refresh(latest); notice.replaceChildren();
    } catch (error) {
      if (api.current() && !accessChanged(error)) reload('Could not read group order.');
    }
  }
  async function move(ids) {
    setBusy(true); notice.textContent = 'Saving group order…';
    try {
      await api.reorderGroups(board().revision, ids);
    } catch (error) {
      if (!api.current() || accessChanged(error)) return;
      if (error === 500 || error === 422 || error === 413) {
        setBusy(false); notice.textContent = 'Could not move group. Reopen its menu to retry.';
      } else reload(error === 409 ? 'Group order changed.' : 'Could not confirm group order.');
      return;
    }
    await readOrder();
  }
}
