// A removal message belongs to this mounted visit only. Its label is never a
// restore target. Both Restore gestures submit the same board-scoped command.
export function groupMenus({api, board, busy, setBusy, refresh, button}, notice) {
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
  return group => {
    const menu = document.createElement('span'); menu.className = 'group-menu';
    const toggle = button('⋯', () => {
      if (busy()) return;
      expand(archive.hidden);
      if (!archive.hidden && !archive.disabled) archive.focus();
    });
    toggle.setAttribute('aria-label', `Group menu for ${group.name}`);
    const archive = button('Archive group', () => { expand(false); submit(group, 'archive'); });
    archive.disabled = !board().owner;
    if (archive.disabled) archive.title = 'Owner access required.';
    function expand(open) { archive.hidden = !open; toggle.setAttribute('aria-expanded', String(open)); }
    menu.onkeydown = e => { if (e.key === 'Escape') { expand(false); toggle.focus(); } };
    menu.onfocusout = e => { if (!menu.contains(e.relatedTarget)) expand(false); };
    expand(false); menu.append(toggle, archive); return menu;
  };
}
