use ely_gpui_component::primitives::Severity;
use gpui::{RenderImage, SharedString};
use std::sync::Arc;

use crate::core::keys::KeyAction;
use crate::core::{GroupId, TabId, UpdateState};
use crate::win::{IconPixels, WindowSnapshot};

#[derive(Debug, Clone)]
pub enum Cmd {
    Refresh,
    HostReady {
        hwnd: isize,
        titlebar: f32,
    },
    SelectTab {
        group: GroupId,
        tab: TabId,
    },
    Ungroup {
        group: GroupId,
        tab: TabId,
    },
    CloseTab {
        group: GroupId,
        tab: TabId,
    },
    DetachTabOut {
        from: usize,
        screen_x: f32,
        screen_y: f32,
    },
    ReorderTab {
        group: GroupId,
        from: usize,
        to: usize,
    },
    BeginPick,
    ToggleIconsInTabs,
    SetOverlay(bool),
    ToggleOverlay,
    SetTabMenu(bool),
    ToggleTabMenu,
    MoveTabMenuCursor {
        delta: isize,
    },
    ConfirmTabMenuCursor,
    CloseOtherTabs {
        group: GroupId,
        tab: TabId,
    },
    CloseTabsToRight {
        group: GroupId,
        tab: TabId,
    },
    CloseTabsToLeft {
        group: GroupId,
        tab: TabId,
    },
    OpenTabMenu {
        tab: TabId,
        at: (f32, f32),
    },
    CloseTabMenu,
    BeginRename {
        group: GroupId,
        tab: TabId,
    },
    CancelRename,
    RenameTab {
        group: GroupId,
        tab: TabId,
        title: String,
    },
    SetPickerOpen(bool),
    TogglePicker,
    TogglePickerPick {
        hwnd: isize,
    },
    MovePickerCursor {
        delta: isize,
    },
    TogglePickerCursor,
    AddPicked,
    SetSavedMenu(bool),
    ToggleSavedMenu,
    BeginSaveSet,
    BeginRenameSet {
        id: u64,
    },
    CancelSavedPrompt,
    CommitSaveSet {
        name: String,
    },
    CommitRenameSet {
        id: u64,
        name: String,
    },
    RestoreSet {
        id: u64,
    },
    DeleteSet {
        id: u64,
    },
    CopyDiagnostics,
    ToggleTabBadge,
    ToggleHoverPreview,
    ToggleTheme,
    ToggleRestoreSession,
    SetTabColor {
        group: GroupId,
        tab: TabId,
        color: Option<usize>,
    },
    PreviewTab {
        tab: TabId,
        anchor: (f32, f32, f32, f32),
    },
    PreviewHide {
        tab: TabId,
    },
    PreviewCardHover(bool),
    DismissPreview,
    Shutdown,
    ToggleTabBar,
    OpenRepository,
    CheckForUpdates {
        force: bool,
    },
    ToggleUpdateChannel,
    OpenSlateFolder,
    ReloadSettings,
    OpenKeybinds,
    CloseDialog,
    BannerAction(BannerAction),
    DismissBanner,
    DismissUpdateBanner,
    SetKeybindChord {
        action: KeyAction,
        chord: Option<crate::core::keys::Chord>,
    },
    ResetKeybinds,
    CommitKeybinds,
    RestoreRecovered,
    DismissRecovered,
}

#[derive(Clone)]
pub struct TabInfo {
    pub id: TabId,
    pub title: String,
    pub app: String,
    pub exe: String,
    pub exe_path: String,
    pub icon: Option<Arc<RenderImage>>,
    pub accent: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BannerAction {
    InstallUpdate,
    RetryUpdateCheck,
    RestartUpdate,
}

/// What a banner says, and which surface renders it.
#[derive(Clone, Debug)]
pub enum BannerKind {
    Message {
        severity: Severity,
        text: SharedString,
        action: Option<(SharedString, BannerAction)>,
    },
    /// The window picker is armed; cleared when it ends.
    Pick,
}

#[derive(Clone, Debug)]
pub struct BannerView {
    pub id: u64,
    pub kind: BannerKind,
}

#[derive(Clone)]
pub struct HostInfo {
    pub group: Option<GroupId>,
    pub tabs: Vec<TabInfo>,
    pub active: Option<TabId>,
    pub icons_in_tabs: bool,
    pub show_tab_badge: bool,
    pub hover_preview_enabled: bool,
    pub theme: SharedString,
    pub restore_session: bool,
    pub overlay: bool,
    pub tab_menu_open: bool,
    pub tab_menu_cursor: Option<usize>,
    pub picker_open: bool,
    pub picker_rows: Vec<PickerRow>,
    pub picker_picks: Vec<isize>,
    pub picker_cursor: Option<usize>,
    pub saved_menu_open: bool,
    pub saved_sets: Vec<SavedSetInfo>,
    pub saved_prompt: Option<SavedPrompt>,
    pub tab_menu: Option<TabMenu>,
    pub renaming: Option<TabId>,
    pub picking: bool,
    pub hover_preview: Option<HoverPreview>,
    pub show_tab_bar: bool,
    pub update: UpdateState,
    /// Set when the user dismissed the update banner for this version.
    pub update_banner_hidden: Option<String>,
    pub banner: Option<BannerView>,
    pub keybind_error: Option<SharedString>,
    pub dialog: Option<Dialog>,
    pub keybinds: crate::config::Keybinds,
    pub caption: SharedString,
    pub version_label: SharedString,
    pub update_label: SharedString,
    pub beta_channel: bool,
}

#[derive(Clone)]
pub struct TabMenu {
    pub tab: TabId,
    pub at: (f32, f32),
}

#[derive(Clone)]
pub struct PickerRow {
    pub hwnd: isize,
    pub title: String,
    pub exe: String,
    pub icon: Option<Arc<RenderImage>>,
    pub reason: Option<String>,
}

#[derive(Clone)]
pub struct SavedSetInfo {
    pub id: u64,
    pub name: String,
    pub available: usize,
    pub total: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialog {
    Keybinds,
    Recovery,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SavedPrompt {
    Save,
    Rename(u64),
}

#[derive(Clone)]
pub struct HoverPreview {
    pub tab: TabId,
    pub anchor: (f32, f32, f32, f32),
    pub image: Option<Arc<RenderImage>>,
}

pub(super) fn banner_message(
    severity: Severity,
    text: impl Into<SharedString>,
    action: Option<(SharedString, BannerAction)>,
) -> BannerKind {
    BannerKind::Message {
        severity,
        text: text.into(),
        action,
    }
}

pub(super) fn app_name(exe_name: &str) -> String {
    let stem = exe_name
        .strip_suffix(".exe")
        .or_else(|| exe_name.strip_suffix(".EXE"))
        .unwrap_or(exe_name)
        .trim();
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => exe_name.to_string(),
    }
}

/// Windows hands back BGRA, which is exactly what `RenderImage` wants, so no swap here.
pub(super) fn render_image(px: IconPixels) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(px.width, px.height, px.bgra)?;
    Some(crate::ui::render_image(buffer))
}

pub(super) fn render_snapshot(snap: WindowSnapshot) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(snap.width, snap.height, snap.bgra)?;
    Some(crate::ui::render_image(buffer))
}
