use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::menu::{
    Menu, MenuBuilder, MenuId, MenuItem, MenuItemBuilder, PredefinedMenuItem, Submenu,
    SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Manager, State, Wry};

pub const NATIVE_MENU_EVENT: &str = "video-creater://native-menu-command";

const APP_SETTINGS: &str = "app.settings";
#[cfg(not(target_os = "macos"))]
const APP_QUIT: &str = "app.quit";
const FILE_NEW_PROJECT: &str = "file.newProject";
const FILE_OPEN_PROJECT: &str = "file.openProject";
const FILE_PROJECT_SETTINGS: &str = "file.projectSettings";
const FILE_IMPORT: &str = "file.importMedia";
const FILE_EXPORT: &str = "file.export";
const EDIT_UNDO: &str = "edit.undo";
const EDIT_REDO: &str = "edit.redo";
const EDIT_SELECT_FORWARD_TRACK: &str = "edit.selectForwardTrack";
const EDIT_SELECT_FORWARD_ALL: &str = "edit.selectForwardAll";
const EDIT_SPLIT: &str = "edit.split";
const EDIT_TRIM_START: &str = "edit.trimStart";
const EDIT_TRIM_END: &str = "edit.trimEnd";
const EDIT_DELETE: &str = "edit.delete";
const EDIT_RIPPLE_DELETE: &str = "edit.rippleDelete";
const VIEW_TAB_AI: &str = "view.tab.ai";
const VIEW_TAB_MEDIA: &str = "view.tab.media";
const VIEW_TAB_AUDIO: &str = "view.tab.audio";
const VIEW_TAB_TEXT: &str = "view.tab.text";
const VIEW_TAB_CAPTIONS: &str = "view.tab.captions";
const VIEW_TAB_EFFECTS: &str = "view.tab.effects";
const HELP_SHORTCUTS: &str = "help.shortcuts";
const HELP_CONNECT_AGENTS: &str = "help.connectAgents";
const HELP_PROJECT_GUIDANCE: &str = "help.projectGuidance";
const HELP_ADVANCED_SETTINGS: &str = "help.advancedSettings";
const HELP_SYSTEM_HEALTH: &str = "help.systemHealth";
const HELP_FEEDBACK: &str = "help.feedback";

/// The editor's left tabs in View menu order, with their `CmdOrCtrl+1…6` accelerators.
const VIEW_TABS: [(&str, &str, &str); 6] = [
    (VIEW_TAB_AI, "AI", "CmdOrCtrl+1"),
    (VIEW_TAB_MEDIA, "Media", "CmdOrCtrl+2"),
    (VIEW_TAB_AUDIO, "Audio", "CmdOrCtrl+3"),
    (VIEW_TAB_TEXT, "Text", "CmdOrCtrl+4"),
    (VIEW_TAB_CAPTIONS, "Captions", "CmdOrCtrl+5"),
    (VIEW_TAB_EFFECTS, "Effects", "CmdOrCtrl+6"),
];

/// Items that act in every view.
#[cfg(test)]
const GLOBAL_ITEM_IDS: [&str; 6] = [
    APP_SETTINGS,
    FILE_NEW_PROJECT,
    FILE_OPEN_PROJECT,
    HELP_ADVANCED_SETTINGS,
    HELP_SYSTEM_HEALTH,
    HELP_FEEDBACK,
];

/// Items enabled only while the editor is the active view; some also need a capability.
const EDITOR_ITEM_IDS: [&str; 21] = [
    FILE_PROJECT_SETTINGS,
    FILE_IMPORT,
    FILE_EXPORT,
    EDIT_UNDO,
    EDIT_REDO,
    EDIT_SELECT_FORWARD_TRACK,
    EDIT_SELECT_FORWARD_ALL,
    EDIT_SPLIT,
    EDIT_TRIM_START,
    EDIT_TRIM_END,
    EDIT_DELETE,
    EDIT_RIPPLE_DELETE,
    VIEW_TAB_AI,
    VIEW_TAB_MEDIA,
    VIEW_TAB_AUDIO,
    VIEW_TAB_TEXT,
    VIEW_TAB_CAPTIONS,
    VIEW_TAB_EFFECTS,
    HELP_SHORTCUTS,
    HELP_CONNECT_AGENTS,
    HELP_PROJECT_GUIDANCE,
];

static MENU_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Shortcuts the web view already handles, with editable-target guards. macOS
/// menus only claim key equivalents the web view does not handle (its keydown
/// handler calls `preventDefault`), so one key press still runs one command.
/// GTK accelerators run before the focused web view, so a bare "S" or "[" would
/// be swallowed while typing in a text field and Ctrl+Z would never reach a
/// field's own undo. Non-macOS menus show these items without accelerators.
fn webview_key_accelerator(accelerator: &'static str) -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some(accelerator)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeMenuView {
    Home,
    Editor,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMenuState {
    pub view: NativeMenuView,
    pub can_import: bool,
    pub can_export: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_split: bool,
    pub can_trim_start: bool,
    pub can_trim_end: bool,
    pub can_delete: bool,
    pub can_ripple_delete: bool,
    pub can_select_forward: bool,
}

/// The first Edit items. The editor view uses custom Undo/Redo items that emit
/// `undo` / `redo` to the editor's global undo (agent batches included). Home
/// and Settings keep macOS's predefined items so text fields keep native undo;
/// GTK muda has no predefined undo, so Linux shows no history items there.
struct EditHistoryItems {
    menu: Submenu<Wry>,
    undo: MenuItem<Wry>,
    redo: MenuItem<Wry>,
    #[cfg(target_os = "macos")]
    native_undo: PredefinedMenuItem<Wry>,
    #[cfg(target_os = "macos")]
    native_redo: PredefinedMenuItem<Wry>,
    #[cfg(not(target_os = "macos"))]
    separator: PredefinedMenuItem<Wry>,
    editor_installed: Mutex<bool>,
}

impl EditHistoryItems {
    /// Swaps the history items in place when the view crosses the editor boundary.
    fn show_editor_items(&self, editor: bool) -> tauri::Result<()> {
        let mut installed = self
            .editor_installed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *installed == editor {
            return Ok(());
        }
        if editor {
            #[cfg(target_os = "macos")]
            {
                self.menu.remove(&self.native_undo)?;
                self.menu.remove(&self.native_redo)?;
            }
            self.menu.insert(&self.undo, 0)?;
            self.menu.insert(&self.redo, 1)?;
            #[cfg(not(target_os = "macos"))]
            self.menu.insert(&self.separator, 2)?;
        } else {
            self.menu.remove(&self.undo)?;
            self.menu.remove(&self.redo)?;
            #[cfg(target_os = "macos")]
            {
                self.menu.insert(&self.native_undo, 0)?;
                self.menu.insert(&self.native_redo, 1)?;
            }
            #[cfg(not(target_os = "macos"))]
            self.menu.remove(&self.separator)?;
        }
        *installed = editor;
        Ok(())
    }
}

pub struct NativeMenuRegistry {
    /// Every item whose enabled state follows the view and editor capabilities.
    editor_items: Vec<MenuItem<Wry>>,
    history: EditHistoryItems,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeMenuCommand {
    OpenSettings,
    OpenProjectSettings,
    OpenAdvancedSettings,
    OpenSystemHealth,
    NewProject,
    OpenProject,
    ImportMedia,
    ExportProject,
    Undo,
    Redo,
    SelectForwardTrack,
    SelectForwardAll,
    Split,
    TrimStart,
    TrimEnd,
    Delete,
    RippleDelete,
    #[serde(rename = "showTab:ai")]
    ShowTabAi,
    #[serde(rename = "showTab:media")]
    ShowTabMedia,
    #[serde(rename = "showTab:audio")]
    ShowTabAudio,
    #[serde(rename = "showTab:text")]
    ShowTabText,
    #[serde(rename = "showTab:captions")]
    ShowTabCaptions,
    #[serde(rename = "showTab:effects")]
    ShowTabEffects,
    OpenShortcuts,
    OpenConnectAgents,
    OpenProjectGuidance,
    SendFeedback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeMenuRequest {
    pub sequence: u64,
    pub command: NativeMenuCommand,
}

fn item(
    app: &AppHandle<Wry>,
    id: &'static str,
    label: &'static str,
    accelerator: Option<&'static str>,
    enabled: bool,
) -> tauri::Result<MenuItem<Wry>> {
    let mut builder = MenuItemBuilder::with_id(id, label).enabled(enabled);
    if let Some(accelerator) = accelerator {
        builder = builder.accelerator(accelerator);
    }
    builder.build(app)
}

/// Whether an editor item is enabled for `state`; every editor item is disabled outside the editor.
fn editor_item_enabled(id: &str, state: &NativeMenuState) -> bool {
    if state.view != NativeMenuView::Editor {
        return false;
    }
    match id {
        FILE_IMPORT => state.can_import,
        FILE_EXPORT => state.can_export,
        EDIT_UNDO => state.can_undo,
        EDIT_REDO => state.can_redo,
        EDIT_SELECT_FORWARD_TRACK | EDIT_SELECT_FORWARD_ALL => state.can_select_forward,
        EDIT_SPLIT => state.can_split,
        EDIT_TRIM_START => state.can_trim_start,
        EDIT_TRIM_END => state.can_trim_end,
        EDIT_DELETE => state.can_delete,
        EDIT_RIPPLE_DELETE => state.can_ripple_delete,
        _ => true,
    }
}

pub fn build_native_menu(app: &AppHandle<Wry>) -> tauri::Result<Menu<Wry>> {
    let settings = item(app, APP_SETTINGS, "Settings…", Some("CmdOrCtrl+,"), true)?;
    #[cfg(target_os = "macos")]
    let app_menu = SubmenuBuilder::new(app, "Video Creater")
        .about(None)
        .separator()
        .item(&settings)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let new_project = item(
        app,
        FILE_NEW_PROJECT,
        "New Project",
        Some("CmdOrCtrl+N"),
        true,
    )?;
    let open_project = item(
        app,
        FILE_OPEN_PROJECT,
        "Open Project…",
        Some("CmdOrCtrl+O"),
        true,
    )?;
    let project_settings = item(app, FILE_PROJECT_SETTINGS, "Project Settings…", None, false)?;
    let import = item(
        app,
        FILE_IMPORT,
        "Import Media…",
        Some("CmdOrCtrl+I"),
        false,
    )?;
    let export = item(app, FILE_EXPORT, "Export…", Some("CmdOrCtrl+E"), false)?;
    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&new_project)
        .item(&open_project)
        .separator()
        .item(&project_settings)
        .item(&import)
        .item(&export);
    // Without an application menu, Settings and Quit live in File, following
    // GNOME/KDE conventions. Predefined Quit is not implemented by GTK muda.
    #[cfg(not(target_os = "macos"))]
    let file_menu = {
        let quit = item(app, APP_QUIT, "Quit", Some("CmdOrCtrl+Q"), true)?;
        file_menu
            .separator()
            .item(&settings)
            .separator()
            .item(&quit)
    };
    let file_menu = file_menu.build()?;

    let undo = item(
        app,
        EDIT_UNDO,
        "Undo",
        webview_key_accelerator("CmdOrCtrl+Z"),
        false,
    )?;
    let redo = item(
        app,
        EDIT_REDO,
        "Redo",
        webview_key_accelerator("Shift+CmdOrCtrl+Z"),
        false,
    )?;
    let select_forward_track = item(
        app,
        EDIT_SELECT_FORWARD_TRACK,
        "Select Forward on Track",
        None,
        false,
    )?;
    let select_forward_all = item(
        app,
        EDIT_SELECT_FORWARD_ALL,
        "Select Forward on All Tracks",
        None,
        false,
    )?;
    let split = item(
        app,
        EDIT_SPLIT,
        "Split",
        webview_key_accelerator("S"),
        false,
    )?;
    let trim_start = item(
        app,
        EDIT_TRIM_START,
        "Trim Start to Playhead",
        webview_key_accelerator("["),
        false,
    )?;
    let trim_end = item(
        app,
        EDIT_TRIM_END,
        "Trim End to Playhead",
        webview_key_accelerator("]"),
        false,
    )?;
    let delete = item(app, EDIT_DELETE, "Delete", None, false)?;
    let ripple_delete = item(
        app,
        EDIT_RIPPLE_DELETE,
        "Ripple Delete",
        webview_key_accelerator("Shift+Backspace"),
        false,
    )?;
    // The menu starts outside the editor; `sync_native_menu_state` swaps the
    // history items in when the editor becomes the active view.
    let edit_menu = SubmenuBuilder::new(app, "Edit");
    // GTK muda has no undo/redo items and its clipboard items only draw an
    // accelerator label without acting, so Linux relies on the web view's own
    // clipboard and undo handling instead of dead menu entries.
    #[cfg(target_os = "macos")]
    let native_undo = PredefinedMenuItem::undo(app, None)?;
    #[cfg(target_os = "macos")]
    let native_redo = PredefinedMenuItem::redo(app, None)?;
    #[cfg(target_os = "macos")]
    let edit_menu = edit_menu
        .item(&native_undo)
        .item(&native_redo)
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .separator();
    let edit_menu = edit_menu
        .item(&select_forward_track)
        .item(&select_forward_all)
        .separator()
        .item(&split)
        .item(&trim_start)
        .item(&trim_end)
        .item(&delete)
        .item(&ripple_delete)
        .build()?;

    let tabs = VIEW_TABS
        .iter()
        .map(|&(id, label, accelerator)| item(app, id, label, Some(accelerator), false))
        .collect::<tauri::Result<Vec<_>>>()?;
    let view_menu = tabs
        .iter()
        .fold(SubmenuBuilder::new(app, "View"), |menu, tab| menu.item(tab));
    // Predefined Fullscreen is macOS-only in muda.
    #[cfg(target_os = "macos")]
    let view_menu = view_menu.separator().fullscreen();
    let view_menu = view_menu.build()?;

    let shortcuts = item(
        app,
        HELP_SHORTCUTS,
        "Keyboard Shortcuts",
        Some("CmdOrCtrl+/"),
        false,
    )?;
    let connect_agents = item(
        app,
        HELP_CONNECT_AGENTS,
        "Connect External Agents…",
        None,
        false,
    )?;
    let project_guidance = item(app, HELP_PROJECT_GUIDANCE, "Project Guidance", None, false)?;
    let advanced_settings = item(
        app,
        HELP_ADVANCED_SETTINGS,
        "Advanced Settings…",
        None,
        true,
    )?;
    let system_health = item(app, HELP_SYSTEM_HEALTH, "System Health", None, true)?;
    let feedback = item(app, HELP_FEEDBACK, "Send Feedback…", None, true)?;
    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&shortcuts)
        .item(&connect_agents)
        .item(&project_guidance)
        .separator()
        .item(&advanced_settings)
        .item(&system_health)
        .separator()
        .item(&feedback);
    #[cfg(not(target_os = "macos"))]
    let help_menu = {
        let about = tauri::menu::AboutMetadataBuilder::new()
            .name(Some("Video Creater"))
            .version(Some(app.package_info().version.to_string()))
            .build();
        help_menu.separator().about(Some(about))
    };
    let help_menu = help_menu.build()?;

    let mut editor_items = vec![
        project_settings,
        import,
        export,
        undo.clone(),
        redo.clone(),
        select_forward_track,
        select_forward_all,
        split,
        trim_start,
        trim_end,
        delete,
        ripple_delete,
    ];
    editor_items.extend(tabs);
    editor_items.extend([shortcuts, connect_agents, project_guidance]);
    debug_assert!(editor_items
        .iter()
        .map(|item| item.id().0.as_str())
        .eq(EDITOR_ITEM_IDS));
    app.manage(NativeMenuRegistry {
        editor_items,
        history: EditHistoryItems {
            menu: edit_menu.clone(),
            undo,
            redo,
            #[cfg(target_os = "macos")]
            native_undo,
            #[cfg(target_os = "macos")]
            native_redo,
            #[cfg(not(target_os = "macos"))]
            separator: PredefinedMenuItem::separator(app)?,
            editor_installed: Mutex::new(false),
        },
    });

    let menu = MenuBuilder::new(app);
    #[cfg(target_os = "macos")]
    let menu = menu.item(&app_menu);
    menu.item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&help_menu)
        .build()
}

#[tauri::command]
pub fn sync_native_menu_state(
    state: NativeMenuState,
    registry: State<'_, NativeMenuRegistry>,
) -> Result<(), String> {
    registry
        .history
        .show_editor_items(state.view == NativeMenuView::Editor)
        .map_err(|error| error.to_string())?;
    for item in &registry.editor_items {
        item.set_enabled(editor_item_enabled(&item.id().0, &state))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn command_for_menu_id(id: &MenuId) -> Option<NativeMenuCommand> {
    Some(match id.0.as_str() {
        APP_SETTINGS => NativeMenuCommand::OpenSettings,
        FILE_PROJECT_SETTINGS => NativeMenuCommand::OpenProjectSettings,
        FILE_NEW_PROJECT => NativeMenuCommand::NewProject,
        FILE_OPEN_PROJECT => NativeMenuCommand::OpenProject,
        FILE_IMPORT => NativeMenuCommand::ImportMedia,
        FILE_EXPORT => NativeMenuCommand::ExportProject,
        EDIT_UNDO => NativeMenuCommand::Undo,
        EDIT_REDO => NativeMenuCommand::Redo,
        EDIT_SELECT_FORWARD_TRACK => NativeMenuCommand::SelectForwardTrack,
        EDIT_SELECT_FORWARD_ALL => NativeMenuCommand::SelectForwardAll,
        EDIT_SPLIT => NativeMenuCommand::Split,
        EDIT_TRIM_START => NativeMenuCommand::TrimStart,
        EDIT_TRIM_END => NativeMenuCommand::TrimEnd,
        EDIT_DELETE => NativeMenuCommand::Delete,
        EDIT_RIPPLE_DELETE => NativeMenuCommand::RippleDelete,
        VIEW_TAB_AI => NativeMenuCommand::ShowTabAi,
        VIEW_TAB_MEDIA => NativeMenuCommand::ShowTabMedia,
        VIEW_TAB_AUDIO => NativeMenuCommand::ShowTabAudio,
        VIEW_TAB_TEXT => NativeMenuCommand::ShowTabText,
        VIEW_TAB_CAPTIONS => NativeMenuCommand::ShowTabCaptions,
        VIEW_TAB_EFFECTS => NativeMenuCommand::ShowTabEffects,
        HELP_SHORTCUTS => NativeMenuCommand::OpenShortcuts,
        HELP_CONNECT_AGENTS => NativeMenuCommand::OpenConnectAgents,
        HELP_PROJECT_GUIDANCE => NativeMenuCommand::OpenProjectGuidance,
        HELP_ADVANCED_SETTINGS => NativeMenuCommand::OpenAdvancedSettings,
        HELP_SYSTEM_HEALTH => NativeMenuCommand::OpenSystemHealth,
        HELP_FEEDBACK => NativeMenuCommand::SendFeedback,
        _ => return None,
    })
}

/// Ids of the custom items that can act in `view`. Undo and Redo exist only in
/// the editor view; elsewhere the Edit menu holds the platform's own items.
#[cfg(test)]
fn menu_item_ids_for_test(view: NativeMenuView) -> Vec<&'static str> {
    let mut ids = GLOBAL_ITEM_IDS.to_vec();
    #[cfg(not(target_os = "macos"))]
    ids.push(APP_QUIT);
    if view == NativeMenuView::Editor {
        ids.extend(EDITOR_ITEM_IDS);
    }
    ids
}

pub fn handle_native_menu_event(app: &AppHandle<Wry>, id: &MenuId) {
    #[cfg(not(target_os = "macos"))]
    if id.0 == APP_QUIT {
        app.exit(0);
        return;
    }
    let Some(command) = command_for_menu_id(id) else {
        return;
    };
    let request = NativeMenuRequest {
        sequence: MENU_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        command,
    };
    let _ = app.emit_to("main", NATIVE_MENU_EVENT, request);
}

#[cfg(test)]
mod tests {
    use super::{
        command_for_menu_id, editor_item_enabled, menu_item_ids_for_test, webview_key_accelerator,
        NativeMenuCommand, NativeMenuRequest, NativeMenuState, NativeMenuView, EDITOR_ITEM_IDS,
        GLOBAL_ITEM_IDS, VIEW_TABS,
    };
    use tauri::menu::MenuId;

    fn editor_state() -> NativeMenuState {
        NativeMenuState {
            view: NativeMenuView::Editor,
            can_import: true,
            can_export: true,
            can_undo: true,
            can_redo: true,
            can_split: true,
            can_trim_start: true,
            can_trim_end: true,
            can_delete: true,
            can_ripple_delete: true,
            can_select_forward: true,
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_keeps_web_view_key_equivalents() {
        for accelerator in [
            "S",
            "[",
            "]",
            "Shift+Backspace",
            "CmdOrCtrl+Z",
            "Shift+CmdOrCtrl+Z",
        ] {
            assert_eq!(webview_key_accelerator(accelerator), Some(accelerator));
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn gtk_menus_never_register_web_view_accelerators() {
        // Ctrl+Z must reach the web view: its keydown handler runs the editor
        // undo, and text fields keep their own undo.
        for accelerator in [
            "S",
            "[",
            "]",
            "Shift+Backspace",
            "CmdOrCtrl+Z",
            "Shift+CmdOrCtrl+Z",
        ] {
            assert_eq!(webview_key_accelerator(accelerator), None);
        }
        let ids = menu_item_ids_for_test(NativeMenuView::Home);
        assert!(ids.contains(&"app.quit"));
        assert!(ids.contains(&"app.settings"));
        // Quit is handled natively and never forwarded to the web view.
        assert_eq!(command_for_menu_id(&MenuId::new("app.quit")), None);
    }

    #[test]
    fn stable_ids_map_to_exact_typed_commands() {
        let expected = [
            ("app.settings", NativeMenuCommand::OpenSettings),
            (
                "file.projectSettings",
                NativeMenuCommand::OpenProjectSettings,
            ),
            ("file.newProject", NativeMenuCommand::NewProject),
            ("file.openProject", NativeMenuCommand::OpenProject),
            ("file.importMedia", NativeMenuCommand::ImportMedia),
            ("file.export", NativeMenuCommand::ExportProject),
            ("edit.undo", NativeMenuCommand::Undo),
            ("edit.redo", NativeMenuCommand::Redo),
            ("edit.split", NativeMenuCommand::Split),
            ("edit.rippleDelete", NativeMenuCommand::RippleDelete),
            ("view.tab.ai", NativeMenuCommand::ShowTabAi),
            ("view.tab.effects", NativeMenuCommand::ShowTabEffects),
            ("help.shortcuts", NativeMenuCommand::OpenShortcuts),
            ("help.connectAgents", NativeMenuCommand::OpenConnectAgents),
            (
                "help.projectGuidance",
                NativeMenuCommand::OpenProjectGuidance,
            ),
            (
                "help.advancedSettings",
                NativeMenuCommand::OpenAdvancedSettings,
            ),
            ("help.systemHealth", NativeMenuCommand::OpenSystemHealth),
            ("help.feedback", NativeMenuCommand::SendFeedback),
        ];

        for (id, command) in expected {
            assert_eq!(command_for_menu_id(&MenuId::new(id)), Some(command));
        }
        for removed in [
            "unknown.command",
            "view.media",
            "view.inspector",
            "view.codex",
            "view.maximize",
            "view.layout.default",
            "view.layout.media",
            "view.layout.vertical",
            "help.tour",
        ] {
            assert_eq!(command_for_menu_id(&MenuId::new(removed)), None);
        }
    }

    #[test]
    fn every_menu_item_emits_the_web_view_command_name() {
        let names = GLOBAL_ITEM_IDS
            .iter()
            .chain(EDITOR_ITEM_IDS.iter())
            .map(|id| {
                let command = command_for_menu_id(&MenuId::new(*id))
                    .unwrap_or_else(|| panic!("{id} has no command"));
                let request = NativeMenuRequest {
                    sequence: 1,
                    command,
                };
                serde_json::to_value(request).expect("request serializes")["command"]
                    .as_str()
                    .expect("command is a string")
                    .to_owned()
            })
            .collect::<Vec<_>>();
        // Mirrors `nativeMenuCommands` in src/lib/native-menu.ts.
        let mut expected = vec![
            "openSettings",
            "newProject",
            "openProject",
            "openAdvancedSettings",
            "openSystemHealth",
            "sendFeedback",
            "openProjectSettings",
            "importMedia",
            "exportProject",
            "undo",
            "redo",
            "selectForwardTrack",
            "selectForwardAll",
            "split",
            "trimStart",
            "trimEnd",
            "delete",
            "rippleDelete",
            "showTab:ai",
            "showTab:media",
            "showTab:audio",
            "showTab:text",
            "showTab:captions",
            "showTab:effects",
            "openShortcuts",
            "openConnectAgents",
            "openProjectGuidance",
        ];
        assert_eq!(names, expected);
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(expected.len(), names.len());
    }

    #[test]
    fn view_menu_lists_the_left_tabs_with_numbered_accelerators() {
        let labels = VIEW_TABS.map(|(_, label, _)| label);
        assert_eq!(
            labels,
            ["AI", "Media", "Audio", "Text", "Captions", "Effects"]
        );
        for (index, (id, _, accelerator)) in VIEW_TABS.iter().enumerate() {
            assert_eq!(*accelerator, format!("CmdOrCtrl+{}", index + 1));
            assert!(EDITOR_ITEM_IDS.contains(id));
        }
    }

    #[test]
    fn editor_items_follow_the_view_and_capabilities() {
        let editor = editor_state();
        for id in EDITOR_ITEM_IDS {
            assert!(editor_item_enabled(id, &editor), "{id} enabled in editor");
            for view in [NativeMenuView::Home, NativeMenuView::Settings] {
                let state = NativeMenuState {
                    view,
                    ..editor.clone()
                };
                assert!(!editor_item_enabled(id, &state), "{id} disabled outside");
            }
        }

        let blocked = NativeMenuState {
            can_import: false,
            can_export: false,
            can_undo: false,
            can_redo: false,
            can_split: false,
            can_trim_start: false,
            can_trim_end: false,
            can_delete: false,
            can_ripple_delete: false,
            can_select_forward: false,
            ..editor
        };
        let still_enabled = EDITOR_ITEM_IDS
            .into_iter()
            .filter(|id| editor_item_enabled(id, &blocked))
            .collect::<Vec<_>>();
        assert_eq!(
            still_enabled,
            [
                "file.projectSettings",
                "view.tab.ai",
                "view.tab.media",
                "view.tab.audio",
                "view.tab.text",
                "view.tab.captions",
                "view.tab.effects",
                "help.shortcuts",
                "help.connectAgents",
                "help.projectGuidance",
            ]
        );
    }

    #[test]
    fn state_parses_the_web_view_shape() {
        let state: NativeMenuState = serde_json::from_value(serde_json::json!({
            "view": "editor",
            "canImport": true,
            "canExport": true,
            "canUndo": true,
            "canRedo": false,
            "canSplit": false,
            "canTrimStart": false,
            "canTrimEnd": false,
            "canDelete": false,
            "canRippleDelete": false,
            "canSelectForward": false,
        }))
        .expect("state parses");
        assert!(state.can_undo && !state.can_redo);
        assert!(
            serde_json::from_value::<NativeMenuState>(serde_json::json!({
                "view": "editor",
                "canImport": true,
            }))
            .is_err()
        );
    }

    #[test]
    fn menu_has_editor_scoped_items_without_dead_update_or_save_items() {
        let editor_ids = menu_item_ids_for_test(NativeMenuView::Editor);
        for id in [
            "file.projectSettings",
            "edit.undo",
            "edit.redo",
            "view.tab.ai",
            "help.shortcuts",
            "help.connectAgents",
            "help.projectGuidance",
            "help.advancedSettings",
            "help.systemHealth",
        ] {
            assert!(editor_ids.contains(&id), "{id}");
        }
        for id in ["app.checkUpdates", "file.save", "file.saveAs", "help.tour"] {
            assert!(!editor_ids.contains(&id), "{id}");
        }

        let home_ids = menu_item_ids_for_test(NativeMenuView::Home);
        for id in [
            "file.projectSettings",
            "edit.undo",
            "edit.redo",
            "view.tab.ai",
            "help.projectGuidance",
        ] {
            assert!(!home_ids.contains(&id), "{id}");
        }
        assert!(home_ids.contains(&"help.advancedSettings"));
        assert!(home_ids.contains(&"help.systemHealth"));
    }
}
