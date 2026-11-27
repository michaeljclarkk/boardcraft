//! Ordered briefs and their cumulative acceptance cases. A branch starts from the
//! last packet whose workflow it actually inherits (not necessarily its predecessor).
type Names = Vec<&'static str>;
pub(crate) const MODEL: &str = "openai/gpt-6.1-sol#xhigh";
pub(crate) const PROBE: &str = "android_emulator_edit";
pub(crate) const PROBE_CASES: &[&str] = &[
    "emulator_rename_then_reopen",
    "emulator_reject_then_correct",
];
pub(crate) const ACCESS: &str = "loopback_board_access";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Device {
    #[default]
    None,
    Probe,
    Frame,
    Edit,
    Delivery,
    SignIn,
    Logout,
    Restore,
    Browse,
}

impl Device {
    pub(crate) fn uses_probe(self) -> bool {
        matches!(
            self,
            Self::Probe | Self::SignIn | Self::Logout | Self::Restore
        )
    }

    pub(crate) fn needs_android_ui(self) -> bool {
        !matches!(self, Self::None | Self::Probe)
    }
}

#[derive(Clone)]
pub(crate) struct Packet {
    pub id: &'static str,
    pub model: &'static str,
    pub database: bool,
    pub target: Option<&'static str>,
    pub cases: Names,
    pub windows: Names,
    pub wasm: bool,
    /// Essential live checks without a reviewed Computer Use observation.
    pub pending_ui: Names,
    pub device: Device,
    pub board_link: bool,
    pub bins: bool,
}

impl Packet {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            model: MODEL,
            database: true,
            target: None,
            cases: vec![],
            windows: vec![],
            wasm: false,
            pending_ui: vec![],
            device: Device::None,
            board_link: false,
            bins: false,
        }
    }
    fn suite(id: &'static str, target: &'static str, cases: &[&'static str]) -> Self {
        Self {
            target: Some(target),
            cases: cases.to_vec(),
            ..Self::new(id)
        }
    }
}

pub(crate) fn packets() -> Vec<Packet> {
    let mut all = vec![
        Packet {
            database: false,
            ..Packet::suite("01-board", "board_edit", &[])
        },
        Packet::suite("02-postgres", "postgres_board", &[]),
        Packet {
            bins: true,
            pending_ui: vec!["desktop_save_close_and_reopen"],
            ..Packet::suite("03-desktop", "desktop_edit", &[])
        },
        Packet::suite(
            "04-mobile-edit-service",
            "mobile_edit_service",
            &["rename_then_reconnect", "reject_then_retry"],
        ),
        Packet {
            device: Device::Probe,
            ..Packet::suite("05-android-emulator-edit", PROBE, PROBE_CASES)
        },
    ];
    for (id, device) in [
        ("06-android-kit-frame", Device::Frame),
        ("07-android-kit-edit", Device::Edit),
        ("08-cross-client-reopen", Device::Delivery),
    ] {
        all.push(Packet {
            device,
            ..Packet::new(id)
        });
    }
    let mut names = vec![
        "rejected_edit_then_save_close_and_reopen",
        "display_edit_cancel_blur_and_enter",
    ];
    for (id, extra, delivery, bins) in [
        ("09-windows-name-cell", None, true, false),
        (
            "10-windows-name-recovery",
            Some("parent_failure_retry_dismiss_and_commit"),
            true,
            true,
        ),
        (
            "11-windows-failure-reopen",
            Some("failed_name_reopens_only_for_actor_and_board"),
            false,
            false,
        ),
        (
            "12-windows-name-draft",
            Some("unsubmitted_name_reopens_with_entry_baseline"),
            false,
            false,
        ),
    ] {
        names.extend(extra);
        all.push(Packet {
            device: if delivery {
                Device::Delivery
            } else {
                Device::None
            },
            bins,
            windows: names.clone(),
            ..Packet::new(id)
        });
    }
    all.push(Packet::suite(
        "13-account-session",
        "account_session",
        &[
            "rejected_credentials_then_sign_in",
            "resolve_retry_and_revoke",
        ],
    ));
    all.push(Packet::suite(
        "14-loopback-board-access",
        ACCESS,
        &[
            "viewer_reads_but_cannot_rename",
            "member_edits_within_scope",
        ],
    ));
    let mut auth = vec!["connected_member_sign_in_reject_then_correct"];
    for (id, device, added) in [
        ("15-android-kit-sign-in", Device::SignIn, None),
        (
            "16-android-kit-logout",
            Device::Logout,
            Some("explicit_logout_isolates_sessions"),
        ),
        (
            "17-android-session-restore",
            Device::Restore,
            Some("stored_bearer_reopens_only_while_authorized"),
        ),
    ] {
        auth.extend(added);
        all.push(Packet {
            device,
            ..Packet::suite(id, ACCESS, &auth)
        });
    }
    names.insert(0, "windows_login_reject_retry");
    all.push(Packet {
        windows: names.clone(),
        ..Packet::new("18-windows-connected-login")
    });
    names.insert(0, "windows_env_session_is_origin_scoped");
    all.push(Packet {
        windows: names.clone(),
        ..Packet::new("19-windows-env-session")
    });
    let mut navigation = Packet::suite(
        "20-windows-connected-name",
        ACCESS,
        &[
            "member_edits_within_scope",
            "viewer_reads_but_cannot_rename",
        ],
    );
    navigation.windows = vec![
        "connected_member_renames_then_reopens",
        "connected_viewer_cannot_rename",
    ];
    navigation.windows.extend(names);
    all.push(navigation.clone());
    for (id, cases, windows) in [
        (
            "21-windows-connected-logout",
            &["explicit_logout_isolates_sessions"][..],
            &[
                "configured_logout_failure_then_retry",
                "interactive_viewer_logs_out",
            ][..],
        ),
        (
            "22-visible-board-list",
            &[
                "list_visible_boards_in_navigation_order",
                "workspace_membership_changes_refresh_list_and_access",
            ],
            &[],
        ),
        (
            "23-windows-board-navigation",
            &["selected_board_read_checks_current_membership"],
            &[
                "opens_selected_board_by_id",
                "revoked_membership_blocks_board_selection",
            ],
        ),
    ] {
        navigation.id = id;
        navigation.cases.extend_from_slice(cases);
        navigation.windows.extend_from_slice(windows);
        all.push(navigation.clone());
    }
    all.push(Packet {
        id: "24-android-board-navigation",
        device: Device::Browse,
        ..navigation.clone()
    });
    let link = Packet {
        id: "25-windows-board-link",
        board_link: true,
        ..navigation.clone()
    };
    all.push(link.clone());
    let browser = Packet {
        id: "26-browser-board-preview",
        wasm: true,
        pending_ui: vec![
            "browser_reject_correct_list_logout",
            "browser_empty_unavailable_then_retry",
        ],
        ..link
    };
    all.push(browser.clone());
    let mut browser = Packet {
        windows: vec![],
        board_link: false,
        ..browser
    };
    for (id, cases, pending) in BROWSER_JOURNEYS {
        browser.id = id;
        browser.cases.extend_from_slice(cases);
        browser.pending_ui.extend_from_slice(pending);
        all.push(browser.clone());
    }
    for (id, host, windows) in NATIVE {
        navigation.id = id;
        navigation.cases.push(host);
        navigation.windows.extend_from_slice(windows);
        if *id == "51-selected-root-archive" {
            navigation.wasm = true;
            navigation.pending_ui.clone_from(&browser.pending_ui);
            navigation
                .pending_ui
                .push("browser_archives_selected_roots");
            for case in &browser.cases {
                if !navigation.cases.contains(case) {
                    navigation.cases.push(case);
                }
            }
        } else if *id == "52-board-item-trash" {
            navigation
                .pending_ui
                .push("browser_deletes_and_restores_board_roots");
        }
        all.push(navigation.clone());
    }
    all.push(Packet {
        wasm: true,
        pending_ui: vec!["shell_session_resolve_retry_and_rejection"],
        ..Packet::suite(
            "53-web-shell",
            ACCESS,
            &["web_browser_session_is_origin_bound"],
        )
    });
    for (id, source) in [
        ("54-web-auth-board", "30-browser-session-resume"),
        ("55-web-groups", "52-board-item-trash"),
        ("56-web-test-harness", "55-web-groups"),
    ] {
        let mut web = all
            .iter()
            .find(|p| p.id == source)
            .expect("registered predecessor")
            .clone();
        web.id = id;
        web.windows.clear();
        all.push(web);
    }
    for p in &mut all {
        if p.device.needs_android_ui() {
            p.pending_ui.push("live_android_kit_journey");
        }
        if p.board_link {
            p.pending_ui.push("attended_clipboard_share_and_open");
        }
    }
    all
}

type AddCases = (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
);
// Last column: outstanding outcomes, not test names to invoke on the retired driver.
const BROWSER_JOURNEYS: &[AddCases] = &[
    (
        "27-browser-board-navigation",
        &[],
        &[
            "browser_opens_selected_board_by_id",
            "browser_revoked_membership_blocks_selection",
        ],
    ),
    (
        "28-browser-board-history",
        &["web_transport_preserves_scoped_native_api"],
        &[
            "browser_history_rechecks_board_access",
            "browser_direct_board_survives_sign_in",
        ],
    ),
    (
        "29-browser-selected-name",
        &["web_selected_rename_requires_current_access"],
        &[
            "browser_member_recovers_selected_name",
            "browser_demoted_viewer_cannot_rename",
        ],
    ),
    (
        "30-browser-session-resume",
        &["web_browser_session_is_origin_bound"],
        &[
            "browser_reload_and_independent_tab_resume",
            "browser_unavailable_then_rejected_resume",
        ],
    ),
    (
        "31-browser-board-groups",
        &[
            "web_groups_require_active_membership",
            "group_migration_preserves_flat_boards",
        ],
        &[
            "browser_opens_and_edits_grouped_board",
            "browser_grouped_board_rechecks_membership",
        ],
    ),
    (
        "32-browser-group-item-create",
        &[
            "web_create_requires_current_group_access",
            "web_create_is_atomic_and_replay_safe",
        ],
        &[
            "browser_creates_item_in_selected_group",
            "browser_demoted_viewer_cannot_create_item",
        ],
    ),
    (
        "33-browser-group-item-move",
        &[
            "web_move_requires_current_group_access",
            "web_move_checks_group_revision_and_order",
        ],
        &[
            "browser_moves_item_by_group_id",
            "browser_demoted_viewer_cannot_move_item",
            "browser_reviews_failed_group_move",
        ],
    ),
    (
        "34-browser-group-item-reorder",
        &[
            "web_reorder_requires_current_access_and_anchor",
            "web_reorder_checks_revisions_and_replay",
        ],
        &[
            "browser_reorders_duplicate_rows_by_pointer",
            "browser_demoted_viewer_cannot_reorder",
            "browser_failed_reorder_rolls_back_then_redrags",
        ],
    ),
    (
        "35-browser-item-selection",
        &[],
        &[
            "browser_selects_duplicate_rows_and_groups",
            "browser_shift_selects_roots_across_groups",
        ],
    ),
    (
        "36-browser-selection-group-move",
        &[
            "web_selection_move_requires_current_access_and_valid_roots",
            "web_selection_move_is_atomic_and_replay_safe",
        ],
        &[
            "browser_moves_toolbar_selection_in_request_order",
            "browser_reviews_conflicting_selection_move",
            "browser_demoted_viewer_cannot_move_selection",
        ],
    ),
    (
        "37-browser-root-archive-restore",
        &[
            "web_root_lifecycle_requires_current_owner",
            "web_root_lifecycle_preserves_visibility_and_replay",
        ],
        &[
            "browser_archives_and_restores_one_root",
            "browser_recovers_failed_root_lifecycle",
            "browser_demoted_member_cannot_restore_root",
        ],
    ),
    (
        "38-browser-group-archive-restore",
        &[
            "web_group_lifecycle_requires_current_owner",
            "web_group_lifecycle_preserves_children_and_order",
        ],
        &[
            "browser_archives_and_restores_one_group",
            "browser_recovers_failed_group_lifecycle",
            "browser_demoted_owner_cannot_restore_group",
        ],
    ),
    (
        "39-browser-group-create",
        &[
            "web_group_create_requires_current_owner",
            "web_group_create_preserves_identity_and_order",
        ],
        &[
            "browser_creates_duplicate_group_in_position_order",
            "browser_reviews_failed_group_create",
            "browser_demoted_owner_cannot_create_group",
        ],
    ),
    (
        "40-browser-group-edit",
        &[
            "web_group_edit_requires_current_owner",
            "web_group_edit_checks_both_revisions_and_preserves_children",
        ],
        &[
            "browser_edits_existing_group_by_id",
            "browser_reviews_conflicting_group_edit",
            "browser_demoted_owner_cannot_edit_group",
        ],
    ),
    (
        "41-browser-group-reorder",
        &[
            "web_group_reorder_requires_current_owner",
            "web_group_reorder_checks_active_permutation_and_commit",
        ],
        &[
            "browser_moves_group_one_place_from_menu",
            "browser_reloads_order_before_moving_group_again",
            "browser_demoted_owner_cannot_reorder_groups",
        ],
    ),
    (
        "42-browser-group-heading-drag",
        &[],
        &[
            "browser_drags_group_across_sections",
            "browser_cancels_group_heading_drag",
            "browser_scrolls_to_group_drop_target",
            "browser_recovers_group_drag_after_refusal",
        ],
    ),
    (
        "43-browser-group-collapse",
        &[
            "web_group_collapse_checks_view_revision_and_scope",
            "web_group_collapse_requires_write_access_and_commit",
        ],
        &[
            "browser_collapses_physical_groups_by_id",
            "browser_collapses_all_and_preserves_order",
            "browser_viewer_temporarily_expands_groups",
            "browser_recovers_group_presentation_save",
        ],
    ),
];

type Suite = (&'static str, &'static str, &'static [&'static str]);
const NATIVE: &[Suite] = &[
    (
        "44-windows-main-table",
        "native_main_table_resolves_current_access",
        &[
            "windows_opens_and_folds_main_table",
            "windows_recovers_main_table_fold",
        ],
    ),
    (
        "45-windows-grouped-item-entry",
        "native_grouped_item_entry_checks_current_access",
        &[
            "windows_creates_and_renames_grouped_root",
            "windows_recovers_grouped_item_entry",
        ],
    ),
    (
        "46-windows-selection-move",
        "native_selection_move_checks_access_and_order",
        &[
            "windows_moves_selected_roots_to_group",
            "windows_reviews_selected_move",
        ],
    ),
    (
        "47-windows-board-archive",
        "native_board_archive_checks_access_and_scope",
        &[
            "windows_archives_and_restores_board_content",
            "windows_recovers_board_archive",
        ],
    ),
    (
        "48-windows-group-fields",
        "native_group_fields_check_current_owner",
        &[
            "windows_creates_and_edits_group_fields",
            "windows_recovers_group_fields",
        ],
    ),
    (
        "49-windows-group-order",
        "native_group_order_checks_current_owner",
        &[
            "windows_reorders_physical_groups",
            "windows_recovers_group_reorder",
        ],
    ),
    (
        "50-windows-item-placement",
        "native_single_root_placement_checks_access",
        &[
            "windows_moves_and_reorders_single_roots",
            "windows_recovers_single_root_placement",
        ],
    ),
    (
        "51-selected-root-archive",
        "selected_root_archive_is_atomic_and_authorized",
        &["windows_archives_selected_roots"],
    ),
    (
        "52-board-item-trash",
        "selected_root_trash_is_atomic_and_authorized",
        &["windows_deletes_and_restores_board_roots"],
    ),
];
