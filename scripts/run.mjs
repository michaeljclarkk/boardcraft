import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const connectedName = { database: true,
  target: 'loopback_board_access', cases: ['member_edits_within_scope', 'viewer_reads_but_cannot_rename'],
  windowsCases: ['connected_member_renames_then_reopens', 'connected_viewer_cannot_rename', 'windows_env_session_is_origin_scoped', 'windows_login_reject_retry', 'rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit', 'failed_name_reopens_only_for_actor_and_board', 'unsubmitted_name_reopens_with_entry_baseline'] };

const connectedLogout = { ...connectedName,
  cases: [...connectedName.cases, 'explicit_logout_isolates_sessions'],
  windowsCases: [...connectedName.windowsCases, 'configured_logout_failure_then_retry', 'interactive_viewer_logs_out'] };

const visibleBoardList = { ...connectedLogout,
  cases: [...connectedLogout.cases, 'list_visible_boards_in_navigation_order', 'workspace_membership_changes_refresh_list_and_access'] };

const boardNavigation = { ...visibleBoardList,
    cases: [...visibleBoardList.cases, 'selected_board_read_checks_current_membership'],
    windowsCases: [...visibleBoardList.windowsCases, 'opens_selected_board_by_id', 'revoked_membership_blocks_board_selection'] };

const previewCases = ['browser_reject_correct_list_logout', 'browser_empty_unavailable_then_retry'];
const previewNavigation = { database: true, target: boardNavigation.target,
  cases: boardNavigation.cases, browserCases: [...previewCases, 'browser_opens_selected_board_by_id', 'browser_revoked_membership_blocks_selection'] };

const previewHistory = { ...previewNavigation,
  cases: [...previewNavigation.cases, 'preview_document_preserves_scoped_api'],
  browserCases: [...previewNavigation.browserCases, 'browser_history_rechecks_board_access', 'browser_direct_board_survives_sign_in'] };

const previewNames = { ...previewHistory, cases: [...previewHistory.cases, 'preview_selected_rename_requires_current_access'],
  browserCases: [...previewHistory.browserCases, 'browser_member_recovers_selected_name', 'browser_demoted_viewer_cannot_rename'] };

const previewResume = { ...previewNames, cases: [...previewNames.cases, 'preview_browser_session_is_origin_bound'],
  browserCases: [...previewNames.browserCases, 'browser_reload_and_independent_tab_resume', 'browser_unavailable_then_rejected_resume'] };

const previewGroups = { ...previewResume,
  cases: [...previewResume.cases, 'preview_groups_require_active_membership', 'group_migration_preserves_flat_boards'],
  browserCases: [...previewResume.browserCases, 'browser_opens_and_edits_grouped_board', 'browser_grouped_board_rechecks_membership'] };

const previewCreate = { ...previewGroups,
  cases: [...previewGroups.cases, 'preview_create_requires_current_group_access', 'preview_create_is_atomic_and_replay_safe'],
  browserCases: [...previewGroups.browserCases, 'browser_creates_item_in_selected_group', 'browser_demoted_viewer_cannot_create_item'] };

const previewMove = { ...previewCreate,
  cases: [...previewCreate.cases, 'preview_move_requires_current_group_access', 'preview_move_checks_group_revision_and_order'],
  browserCases: [...previewCreate.browserCases, 'browser_moves_item_by_group_id', 'browser_demoted_viewer_cannot_move_item', 'browser_reviews_failed_group_move'] };

const previewReorder = { ...previewMove,
  cases: [...previewMove.cases, 'preview_reorder_requires_current_access_and_anchor', 'preview_reorder_checks_revisions_and_replay'],
  browserCases: [...previewMove.browserCases, 'browser_reorders_duplicate_rows_by_pointer', 'browser_demoted_viewer_cannot_reorder', 'browser_failed_reorder_rolls_back_then_redrags'] };

const previewSelection = { ...previewReorder,
  browserCases: [...previewReorder.browserCases, 'browser_selects_duplicate_rows_and_groups', 'browser_shift_selects_roots_across_groups'] };

const previewSelectionMove = { ...previewSelection,
  cases: [...previewSelection.cases, 'preview_selection_move_requires_current_access_and_valid_roots', 'preview_selection_move_is_atomic_and_replay_safe'],
  browserCases: [...previewSelection.browserCases, 'browser_moves_toolbar_selection_in_request_order', 'browser_reviews_conflicting_selection_move', 'browser_demoted_viewer_cannot_move_selection'] };

const previewRootLifecycle = { ...previewSelectionMove,
  cases: [...previewSelectionMove.cases, 'preview_root_lifecycle_requires_current_owner', 'preview_root_lifecycle_preserves_visibility_and_replay'],
  browserCases: [...previewSelectionMove.browserCases, 'browser_archives_and_restores_one_root', 'browser_recovers_failed_root_lifecycle', 'browser_demoted_member_cannot_restore_root'] };

const previewGroupLifecycle = { ...previewRootLifecycle,
  cases: [...previewRootLifecycle.cases, 'preview_group_lifecycle_requires_current_owner', 'preview_group_lifecycle_preserves_children_and_order'],
  browserCases: [...previewRootLifecycle.browserCases, 'browser_archives_and_restores_one_group', 'browser_recovers_failed_group_lifecycle', 'browser_demoted_owner_cannot_restore_group'] };

const previewGroupCreate = { ...previewGroupLifecycle,
  cases: [...previewGroupLifecycle.cases, 'preview_group_create_requires_current_owner', 'preview_group_create_preserves_identity_and_order'],
  browserCases: [...previewGroupLifecycle.browserCases, 'browser_creates_duplicate_group_in_position_order', 'browser_reviews_failed_group_create', 'browser_demoted_owner_cannot_create_group'] };

const previewGroupEdit = { ...previewGroupCreate,
  cases: [...previewGroupCreate.cases, 'preview_group_edit_requires_current_owner', 'preview_group_edit_checks_both_revisions_and_preserves_children'],
  browserCases: [...previewGroupCreate.browserCases, 'browser_edits_existing_group_by_id', 'browser_reviews_conflicting_group_edit', 'browser_demoted_owner_cannot_edit_group'] };

const previewGroupOrder = { ...previewGroupEdit,
  cases: [...previewGroupEdit.cases, 'preview_group_reorder_requires_current_owner', 'preview_group_reorder_checks_active_permutation_and_commit'],
  browserCases: [...previewGroupEdit.browserCases, 'browser_moves_group_one_place_from_menu', 'browser_reloads_order_before_moving_group_again', 'browser_demoted_owner_cannot_reorder_groups'] };

const previewHeadingDrag = { ...previewGroupOrder,
  browserCases: [...previewGroupOrder.browserCases, 'browser_drags_group_across_sections', 'browser_cancels_group_heading_drag', 'browser_scrolls_to_group_drop_target', 'browser_recovers_group_drag_after_refusal'] };

export const packets = [
  { id: '01-board', target: 'board_edit', database: false },
  { id: '02-postgres', target: 'postgres_board', database: true },
  { id: '03-desktop', target: 'desktop_edit', database: true, buildBins: true },
  { id: '04-mobile-edit-service', target: 'mobile_edit_service', database: true,
    cases: ['rename_then_reconnect', 'reject_then_retry'] },
  { id: '05-android-emulator-edit', target: 'android_emulator_edit', database: true, android: true,
    cases: ['emulator_rename_then_reopen', 'emulator_reject_then_correct'] },
  { id: '06-android-kit-frame', database: true, kitFrame: true },
  { id: '07-android-kit-edit', database: true, kitFrame: true, kitEdit: true },
  { id: '08-cross-client-reopen', database: true, kitFrame: true, kitEdit: true, delivery: true },
  { id: '09-windows-name-cell', database: true, kitFrame: true, kitEdit: true, delivery: true,
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter'] },
  { id: '10-windows-name-recovery', database: true, buildBins: true, kitFrame: true, kitEdit: true, delivery: true,
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit'] },
  { id: '11-windows-failure-reopen', database: true,
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit', 'failed_name_reopens_only_for_actor_and_board'] },
  { id: '12-windows-name-draft', database: true,
    windowsCases: ['rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit', 'failed_name_reopens_only_for_actor_and_board', 'unsubmitted_name_reopens_with_entry_baseline'] },
  { id: '13-account-session', target: 'account_session', database: true,
    cases: ['rejected_credentials_then_sign_in', 'resolve_retry_and_revoke'] },
  { id: '14-loopback-board-access', target: 'loopback_board_access', database: true,
    cases: ['viewer_reads_but_cannot_rename', 'member_edits_within_scope'] },
  { id: '15-android-kit-sign-in', database: true, kitFrame: true, kitEdit: true, delivery: true,
    probeRegression: true },
  { id: '16-android-kit-logout', database: true, kitFrame: true, kitEdit: true, delivery: true,
    probeRegression: true, logout: true },
  { id: '17-android-session-restore', database: true, kitFrame: true, kitEdit: true, delivery: true,
    probeRegression: true, logout: true, restore: true },
  { id: '18-windows-connected-login', database: true,
    windowsCases: ['windows_login_reject_retry', 'rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit', 'failed_name_reopens_only_for_actor_and_board', 'unsubmitted_name_reopens_with_entry_baseline'] },
  { id: '19-windows-env-session', database: true,
    windowsCases: ['windows_env_session_is_origin_scoped', 'windows_login_reject_retry', 'rejected_edit_then_save_close_and_reopen', 'display_edit_cancel_blur_and_enter', 'parent_failure_retry_dismiss_and_commit', 'failed_name_reopens_only_for_actor_and_board', 'unsubmitted_name_reopens_with_entry_baseline'] },
  { id: '20-windows-connected-name', ...connectedName },
  { id: '21-windows-connected-logout', ...connectedLogout },
  { id: '22-visible-board-list', ...visibleBoardList },
  { id: '23-windows-board-navigation', ...boardNavigation },
  { id: '24-android-board-navigation', ...boardNavigation, kitBrowse: true },
  { id: '25-windows-board-link', ...boardNavigation, boardLink: true },
  { id: '26-browser-board-preview', ...boardNavigation, boardLink: true,
    browserCases: previewCases },
  { id: '27-browser-board-navigation', ...previewNavigation },
  { id: '28-browser-board-history', ...previewHistory },
  { id: '29-browser-selected-name', ...previewNames },
  { id: '30-browser-session-resume', ...previewResume },
  { id: '31-browser-board-groups', ...previewGroups },
  { id: '32-browser-group-item-create', ...previewCreate },
  { id: '33-browser-group-item-move', ...previewMove },
  { id: '34-browser-group-item-reorder', ...previewReorder },
  { id: '35-browser-item-selection', ...previewSelection },
  { id: '36-browser-selection-group-move', ...previewSelectionMove },
  { id: '37-browser-root-archive-restore', ...previewRootLifecycle },
  { id: '38-browser-group-archive-restore', ...previewGroupLifecycle },
  { id: '39-browser-group-create', ...previewGroupCreate },
  { id: '40-browser-group-edit', ...previewGroupEdit },
  { id: '41-browser-group-reorder', ...previewGroupOrder },
  { id: '42-browser-group-heading-drag', ...previewHeadingDrag },
  { id: '43-browser-group-collapse', ...previewHeadingDrag,
    cases: [...previewHeadingDrag.cases, 'preview_group_collapse_checks_view_revision_and_scope', 'preview_group_collapse_requires_write_access_and_commit'],
    browserCases: [...previewHeadingDrag.browserCases, 'browser_collapses_physical_groups_by_id', 'browser_collapses_all_and_preserves_order', 'browser_viewer_temporarily_expands_groups', 'browser_recovers_group_presentation_save'] },
  { id: '44-windows-main-table', ...boardNavigation,
    cases: [...boardNavigation.cases, 'native_main_table_resolves_current_access'],
    windowsCases: [...boardNavigation.windowsCases, 'windows_opens_and_folds_main_table', 'windows_recovers_main_table_fold'] },
  { id: '45-windows-grouped-item-entry', ...boardNavigation,
    cases: [...boardNavigation.cases, 'native_main_table_resolves_current_access', 'native_grouped_item_entry_checks_current_access'],
    windowsCases: [...boardNavigation.windowsCases, 'windows_opens_and_folds_main_table', 'windows_recovers_main_table_fold', 'windows_creates_and_renames_grouped_root', 'windows_recovers_grouped_item_entry'] },
].map(packet => ({ model: 'openai/gpt-6.1-sol#xhigh', ...packet }));
const root = fileURLToPath(new URL('../', import.meta.url));
const read = path => JSON.parse(readFileSync(path, 'utf8'));
const text = value => typeof value === 'string' && value.trim().length > 0;

function command(bin, args, capture = false, timeout = 2 * 60 * 60 * 1000) {
  const result = spawnSync(bin, args, { stdio: capture ? ['ignore', 'pipe', 'inherit'] : ['ignore', 'inherit', 'inherit'], encoding: 'utf8', timeout });
  if (result.error) throw result.error;
  if (capture) process.stdout.write(result.stdout ?? '');
  if (result.status !== 0) throw Error(`${bin} stopped (${result.signal ?? result.status})`);
  return result.stdout;
}

// Only probe a selected, already-booted disposable AVD. Never enumerate or select a device by default.
export function requireEditEmulator(exec = command, serial = process.env.RUSTDAY_ANDROID_SERIAL) {
  if (!/^emulator-[0-9]+$/.test(serial ?? '')) throw Error('Set RUSTDAY_ANDROID_SERIAL to the dedicated emulator serial');
  const adb = (...args) => exec('adb', ['-s', serial, ...args], true, 10_000).trim();
  if (adb('emu', 'avd', 'name').split('\n')[0].trim() !== 'Rustday_Edit_X64') throw Error('Expected Rustday_Edit_X64 AVD');
  if (adb('shell', 'getprop', 'sys.boot_completed') !== '1') throw Error('Emulator has not booted');
  if (adb('shell', 'getprop', 'ro.product.cpu.abi') !== 'x86_64') throw Error('Expected x86_64 Android');
}

export function requireKitEmulator(exec = command, env = process.env) {
  const port = env.ANDROID_ADB_SERVER_PORT;
  if (!/^[1-9]\d{0,4}$/.test(port ?? '') || Number(port) > 65535 || port === '5037' ||
      env.ADB_SERVER_SOCKET !== `tcp:${port}` || env.ADB_USB !== '0' || env.ADB_EMU !== '0') {
    throw Error('Expected private loopback adb server with discovery disabled');
  }
  requireEditEmulator(exec, env.RUSTDAY_ANDROID_SERIAL ?? '');
}

export function verify({ target, buildBins, cases = [], android, kitBrowse, kitFrame, kitEdit, delivery, probeRegression, windowsCases, logout, restore, boardLink, browserCases }, exec = command) {
  exec('cargo', ['fmt', '--all', '--', '--check']);
  if (browserCases) {
    const suite = 'browser_preview';
    const args = ['test', '--locked', '-p', 'boardcraft-mobile-edit-host', '--features', 'browser-smoke',
      '--test', suite, '--', '--test-threads=1'];
    requireCases(suite, browserCases, exec('cargo', args, true));
    if (!windowsCases) exec('node', ['--test', 'scripts/run.test.mjs']);
  }
  const windows = () => {
    const args = ['test', '--locked', '-p', 'boardcraft-desktop', '--features', 'native-smoke', '--test', 'windows_smoke'];
    if (windowsCases) requireCases('windows_smoke', windowsCases, exec('cargo', [...args, '--', '--test-threads=1'], true));
    else exec('cargo', args);
  };
  if (windowsCases && !kitFrame) {
    if (boardLink) {
      const name = 'connected::board_link::tests::only_a_canonical_board_at_the_connected_origin';
      requireCases('board address', [name], exec('cargo', ['test','--locked','-p','boardcraft-desktop','--bin','boardcraft-desktop',name,'--','--exact'], true));
      console.log('SKIPPED in unattended runner: two Windows clipboard cases (see packet handoff for attended evidence).');
    }
    exec('node', ['--test', 'scripts/run.test.mjs']);
    if (target) {
      requireCases(target, cases, exec('cargo', ['test', '--locked', '--workspace', '--test', target], true));
      exec('cargo', ['test', '--locked', '--workspace']);
    } else exec('cargo', ['test', '--locked', '-p', 'boardcraft-desktop']);
    windows();
    if (kitBrowse) {
      exec('cargo', ['fmt', '--manifest-path', 'android/Cargo.toml', '--', '--check']);
      exec('node', ['scripts/android-kit-frame.mjs']);
      exec('node', ['scripts/android-kit-edit.mjs']);
      exec('node', ['scripts/android-kit-edit.mjs', '--browse']);
    }
    return;
  }
  if (kitFrame) {
    if (restore) exec('node', ['scripts/cross-client-reopen.mjs', '--restore']);
    if (logout) {
      const suite = 'loopback_board_access';
      requireCases(suite, ['explicit_logout_isolates_sessions'], exec('cargo', ['test','--locked','--workspace','--test',suite], true));
      exec('node', ['scripts/android-kit-edit.mjs', '--logout']);
    }
    if (probeRegression) runDeviceProbe(packets.find(p => p.id === '05-android-emulator-edit'), exec);
    exec('cargo', ['fmt', '--manifest-path', 'android/Cargo.toml', '--', '--check']);
    exec('node', ['scripts/android-kit-frame.mjs']);
    if (kitEdit) {
      exec('node', ['--test', 'scripts/run.test.mjs']);
      // The delivery gate drives every 07 step before continuing on the same fixture.
      exec('node', [delivery ? 'scripts/cross-client-reopen.mjs' : 'scripts/android-kit-edit.mjs']);
      if (delivery) windows();
    }
    exec('cargo', ['test', '--locked', '--workspace']);
    if (buildBins) exec('cargo', ['build', '--locked', '--workspace', '--bins']);
    return;
  }
  if (android) runDeviceProbe({ target, cases }, exec);
  else requireCases(target, cases, exec('cargo', ['test', '--locked', '--workspace', '--test', target], true));
  exec('cargo', ['test', '--locked', '--workspace']);
  if (buildBins) exec('cargo', ['build', '--locked', '--workspace', '--bins']);
}

function runDeviceProbe({ target, cases }, exec) {
  requireEditEmulator(exec);
  exec('cargo', ['build', '--locked', '-p', 'boardcraft-mobile-edit-client', '--bin', 'android-edit-probe', '--target', 'x86_64-linux-android']);
  requireCases(target, cases, exec('cargo', ['test', '--locked', '-p', 'boardcraft-mobile-edit-host', '--features', 'android-smoke', '--test', target, '--', '--test-threads=1'], true, 600_000));
}

function requireCases(target, cases, output) {
  if (!/test result: ok\. [1-9]\d* passed; 0 failed; 0 ignored/.test(output)) throw Error(`No passing, enabled ${target} suite`);
  const lines = output.split(/\r?\n/);
  for (const name of cases) {
    if (!lines.includes(`test ${name} ... ok`)) throw Error(`Missing passing ${target} case: ${name}`);
  }
}

export function validLimit(count) {
  return /^[1-9]\d*$/.test(count) && Number(count) <= packets.length;
}

function stateDirectory() {
  if (!existsSync('.rustday')) return '.boardcraft';
  if (existsSync('.boardcraft')) throw Error('Both .rustday and .boardcraft runner states exist; resolve the conflict before continuing.');
  return '.rustday';
}

export function runQueue(limit, work, check) {
  if (!validLimit(limit)) throw Error(`Expected 1-${packets.length} packets`);
  const state = stateDirectory();
  mkdirSync(state, { recursive: true });
  mkdirSync('handoffs', { recursive: true });
  // Exclusive create: a second runner must never dispatch the same packet.
  writeFileSync(`${state}/lock`, `${process.pid}\n`, { flag: 'wx' });
  try {
    for (const packet of packets) {
      const { id } = packet;
      if (existsSync(`${state}/${id}.ok`)) continue;
      if (limit-- === 0) break;
      const path = `handoffs/${id}.json`;
      const prior = existsSync(path) ? readFileSync(path, 'utf8') : null;
      if (prior !== null) { writeFileSync(`${state}/${id}.previous.json`, prior); rmSync(path); }
      work(packet, state);
      const handoff = read(path);
      if (handoff.status !== 'ready') throw Error(`${id}: ${handoff.summary || 'incomplete'}`);
      if (!text(handoff.summary) || !text(handoff.next) || !Array.isArray(handoff.evidence) || !handoff.evidence.length || !handoff.evidence.every(text)) throw Error(`Incomplete ${path}`);
      check(packet);
      writeFileSync(`${state}/${id}.ok`, new Date().toISOString(), { flag: 'wx' });
      console.log(`Verified ${id}`);
    }
  } finally { rmSync(`${state}/lock`); }
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.chdir(root);
  const [mode = 'status', count, ...extra] = process.argv.slice(2);
  try {
    if (mode === 'status' && count === undefined) {
      const state = stateDirectory();
      for (const { id } of packets) console.log(id, existsSync(`${state}/${id}.ok`) ? 'verified' : 'pending');
    } else if (mode === 'run' && validLimit(count) && !extra.length) {
      const cli = process.env.OPENCODE_BIN || 'opencode';
      command('cargo', ['--version']);
      command(cli, ['--version']);
      runQueue(Number(count), ({ id, database, android, kitFrame, kitBrowse, model }, state) => {
        if (database && !process.env.RUSTDAY_TEST_DATABASE_URL) throw Error('Set RUSTDAY_TEST_DATABASE_URL to the disposable boardcraft DB.');
        if (android) requireEditEmulator();
        if (kitFrame || kitBrowse) requireKitEmulator();
        const brief = `packets/${id}.md`;
        if (!existsSync(brief)) throw Error(`Missing workflow brief: ${brief}`);
        command(cli, ['run', '--standalone', '--agent', 'build', '--model', model, '--title', `boardcraft ${id}`, `Implement ${brief} under AGENTS.md. Reuse existing work. Run focused checks; the runner does full verification once. Update only the affected workflow row and handoffs/${id}.json for documentation. Stop after the handoff.`]);
      }, verify);
    } else throw Error(`Usage: node scripts/run.mjs status | run <1-${packets.length}>`);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
