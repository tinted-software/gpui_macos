//! Finder state: navigation, directory tree, selection, search.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, Modifiers, ScrollStrategy,
    Subscription, UniformListScrollHandle, Window, actions,
};
use gpui_ce_elements::editable_text::{EditableTextState, StringStorage, TextChanged};

use crate::{
    fsx::{self, Entry, FolderSize, Listing, SortKey},
    theme::Theme,
};

actions!(
    finder,
    [
        GoBack,
        GoForward,
        GoUp,
        OpenSelected,
        SelectNext,
        SelectPrevious,
        SelectLeft,
        SelectRight,
        ExpandOrDescend,
        CollapseOrAscend,
        SelectEverything,
        ToggleHidden,
        FocusSearch,
        ClearSearch,
        ShowList,
        ShowIcons,
        Refresh,
        Quit,
        CloseWindow,
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Icons,
    List,
}

pub struct Row {
    pub entry: Arc<Entry>,
    pub depth: usize,
}

pub struct SidebarItem {
    pub label: String,
    pub path: PathBuf,
    pub icon: &'static str,
}

pub struct SidebarSection {
    pub title: &'static str,
    pub items: Vec<SidebarItem>,
}

struct DirState {
    entries: Vec<Arc<Entry>>,
    error: Option<String>,
    denied: bool,
    mtime: Option<SystemTime>,
}

pub struct Finder {
    pub focus: FocusHandle,
    pub search: Entity<EditableTextState>,
    _search_sub: Subscription,
    pub query: String,

    pub cwd: PathBuf,
    history: Vec<PathBuf>,
    history_ix: usize,

    pub view: ViewMode,
    pub sort_key: SortKey,
    pub sort_asc: bool,
    pub show_hidden: bool,

    expanded: HashSet<PathBuf>,
    dirs: HashMap<PathBuf, DirState>,
    pub rows: Vec<Row>,

    pub selection: HashSet<PathBuf>,
    anchor: Option<PathBuf>,
    cursor: Option<PathBuf>,

    pub list_scroll: UniformListScrollHandle,
    pub icon_cols: usize,
    pub folder_sizes: HashMap<PathBuf, FolderSize>,

    search_task: Option<gpui::Task<()>>,
    search_gen: u64,
    search_results: Vec<Arc<Entry>>,
    pub searching: bool,

    pub sidebar: Vec<SidebarSection>,
    pub theme: Theme,
    pub applied_title: Option<PathBuf>,
}

impl Focusable for Finder {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Finder {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let home = home_dir();
        let start = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| {
                let desktop = home.join("Desktop");
                if desktop.is_dir() {
                    desktop
                } else {
                    home.clone()
                }
            });

        let search = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let search_sub = cx.subscribe(&search, |this, state, _: &TextChanged, cx| {
            let query = state.read(cx).as_str().trim().to_string();
            if query != this.query {
                this.query = query;
                this.run_search(cx);
            }
        });

        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        let mut this = Self {
            focus,
            search,
            _search_sub: search_sub,
            query: String::new(),
            cwd: start.clone(),
            history: vec![start],
            history_ix: 0,
            view: ViewMode::List,
            sort_key: SortKey::Name,
            sort_asc: true,
            show_hidden: false,
            expanded: HashSet::new(),
            dirs: HashMap::new(),
            rows: Vec::new(),
            selection: HashSet::new(),
            anchor: None,
            cursor: None,
            list_scroll: UniformListScrollHandle::new(),
            icon_cols: 1,
            folder_sizes: HashMap::new(),
            search_task: None,
            search_gen: 0,
            search_results: Vec::new(),
            searching: false,
            sidebar: build_sidebar(&home),
            theme: Theme::for_window(window),
            applied_title: None,
        };
        this.rebuild_rows();

        // Pick up external changes to the visible folders.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                if this
                    .update(cx, |this, cx| this.refresh_if_changed(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        this
    }

    // -- directory cache ----------------------------------------------------

    fn load_dir(&mut self, path: &Path) {
        let state = match fsx::read_dir(path, self.show_hidden) {
            Ok(Listing { mut entries, mtime }) => {
                fsx::sort_entries(&mut entries, self.sort_key, self.sort_asc);
                DirState { entries, error: None, denied: false, mtime }
            }
            Err(err) => DirState {
                entries: Vec::new(),
                denied: err.kind() == std::io::ErrorKind::PermissionDenied,
                error: Some(match err.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        "macOS is blocking access to this folder. Grant this app Full Disk Access in System Settings, then reopen it.".to_string()
                    }
                    std::io::ErrorKind::NotFound => "This folder no longer exists.".to_string(),
                    _ => err.to_string(),
                }),
                mtime: None,
            },
        };
        self.dirs.insert(path.to_path_buf(), state);
    }

    pub fn cwd_error(&self) -> Option<&str> {
        self.dirs.get(&self.cwd).and_then(|d| d.error.as_deref())
    }

    pub fn cwd_denied(&self) -> bool {
        self.dirs.get(&self.cwd).is_some_and(|d| d.denied)
    }

    pub fn is_empty_folder(&self) -> bool {
        self.rows.is_empty() && self.query.is_empty() && self.cwd_error().is_none()
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        if !self.query.is_empty() {
            self.rows.extend(self.search_results.iter().map(|e| Row {
                entry: e.clone(),
                depth: 0,
            }));
        } else {
            let cwd = self.cwd.clone();
            self.push_dir(&cwd, 0);
        }
        // Drop selection entries that no longer exist in the visible rows.
        let visible: HashSet<&PathBuf> = self.rows.iter().map(|r| &r.entry.path).collect();
        self.selection.retain(|p| visible.contains(p));
        if self.cursor.as_ref().is_some_and(|c| !visible.contains(c)) {
            self.cursor = None;
        }
        if self.anchor.as_ref().is_some_and(|c| !visible.contains(c)) {
            self.anchor = None;
        }
    }

    fn push_dir(&mut self, path: &Path, depth: usize) {
        if !self.dirs.contains_key(path) {
            self.load_dir(path);
        }
        let entries = self.dirs[path].entries.clone();
        for entry in entries {
            let expand =
                self.view == ViewMode::List && entry.is_dir && self.expanded.contains(&entry.path);
            let child_path = entry.path.clone();
            self.rows.push(Row { entry, depth });
            if expand {
                self.push_dir(&child_path, depth + 1);
            }
        }
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    fn reload_all(&mut self) {
        let paths: Vec<PathBuf> = self.dirs.keys().cloned().collect();
        self.dirs.clear();
        for path in paths {
            if path == self.cwd || self.expanded.contains(&path) {
                self.load_dir(&path);
            }
        }
        self.rebuild_rows();
    }

    fn refresh_if_changed(&mut self, cx: &mut Context<Self>) {
        let changed = self.dirs.iter().any(|(path, state)| {
            let now = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            now != state.mtime
        });
        if changed {
            self.reload_all();
            if !self.query.is_empty() {
                self.run_search(cx);
            }
            cx.notify();
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.reload_all();
        if !self.query.is_empty() {
            self.run_search(cx);
        }
        cx.notify();
    }

    // -- navigation ---------------------------------------------------------

    pub fn navigate(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate_selecting(path, None, cx);
    }

    fn navigate_selecting(
        &mut self,
        path: PathBuf,
        select: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if path == self.cwd {
            return;
        }
        self.history.truncate(self.history_ix + 1);
        self.history.push(path.clone());
        self.history_ix = self.history.len() - 1;
        self.enter(path, select, cx);
    }

    fn enter(&mut self, path: PathBuf, select: Option<PathBuf>, cx: &mut Context<Self>) {
        self.cwd = path;
        self.expanded.clear();
        self.dirs.clear();
        self.selection.clear();
        self.anchor = None;
        self.cursor = None;
        self.clear_search_state(cx);
        self.rebuild_rows();
        if let Some(select) = select
            && let Some(ix) = self.index_of(&select)
        {
            self.select_only(ix);
            self.scroll_to(ix, ScrollStrategy::Center);
        } else {
            self.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        cx.notify();
    }

    fn clear_search_state(&mut self, cx: &mut Context<Self>) {
        self.query.clear();
        self.search_results.clear();
        self.search_task = None;
        self.searching = false;
        self.search_gen += 1;
        self.search.update(cx, |state, cx| {
            if !state.as_str().is_empty() {
                state.emplace("", cx);
            }
        });
    }

    pub fn can_go_back(&self) -> bool {
        self.history_ix > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_ix + 1 < self.history.len()
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.can_go_back() {
            self.history_ix -= 1;
            let path = self.history[self.history_ix].clone();
            self.enter(path, None, cx);
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.can_go_forward() {
            self.history_ix += 1;
            let path = self.history[self.history_ix].clone();
            self.enter(path, None, cx);
        }
    }

    pub fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.cwd.parent() {
            let from = self.cwd.clone();
            self.navigate_selecting(parent.to_path_buf(), Some(from), cx);
        }
    }

    pub fn open_entry(&mut self, entry: &Entry, cx: &mut Context<Self>) {
        if entry.is_dir {
            self.navigate(entry.path.clone(), cx);
        } else {
            cx.open_with_system(&entry.path);
        }
    }

    pub fn open_selected(&mut self, cx: &mut Context<Self>) {
        let chosen: Vec<Arc<Entry>> = self
            .rows
            .iter()
            .filter(|r| self.selection.contains(&r.entry.path))
            .map(|r| r.entry.clone())
            .collect();
        match chosen.as_slice() {
            [] => {}
            [one] => self.open_entry(one, cx),
            many => {
                for entry in many.iter().filter(|e| !e.is_dir) {
                    cx.open_with_system(&entry.path);
                }
            }
        }
    }

    // -- view options -------------------------------------------------------

    pub fn set_view(&mut self, view: ViewMode, cx: &mut Context<Self>) {
        if self.view != view {
            self.view = view;
            self.rebuild_rows();
            cx.notify();
        }
    }

    pub fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.reload_all();
        if !self.query.is_empty() {
            self.run_search(cx);
        }
        cx.notify();
    }

    pub fn sort_by(&mut self, key: SortKey, cx: &mut Context<Self>) {
        if self.sort_key == key {
            self.sort_asc = !self.sort_asc;
        } else {
            self.sort_key = key;
            self.sort_asc = true;
        }
        for state in self.dirs.values_mut() {
            fsx::sort_entries(&mut state.entries, self.sort_key, self.sort_asc);
        }
        fsx::sort_entries(&mut self.search_results, self.sort_key, self.sort_asc);
        self.rebuild_rows();
        cx.notify();
    }

    // -- expansion ----------------------------------------------------------

    pub fn toggle_expanded(&mut self, path: &Path, cx: &mut Context<Self>) {
        if self.view != ViewMode::List || !self.query.is_empty() {
            return;
        }
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_path_buf());
        }
        self.rebuild_rows();
        cx.notify();
    }

    pub fn expand_or_descend(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewMode::Icons {
            return self.move_cursor(1, false, cx);
        }
        let Some(ix) = self.cursor_ix() else { return };
        let entry = self.rows[ix].entry.clone();
        if entry.is_dir && self.query.is_empty() {
            if !self.expanded.contains(&entry.path) {
                self.toggle_expanded(&entry.path, cx);
            } else if ix + 1 < self.rows.len() && self.rows[ix + 1].depth > self.rows[ix].depth {
                self.select_only(ix + 1);
                self.scroll_to(ix + 1, ScrollStrategy::Nearest);
                cx.notify();
            }
        }
    }

    pub fn collapse_or_ascend(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewMode::Icons {
            return self.move_cursor(-1, false, cx);
        }
        let Some(ix) = self.cursor_ix() else { return };
        let entry = self.rows[ix].entry.clone();
        if entry.is_dir && self.expanded.contains(&entry.path) {
            self.toggle_expanded(&entry.path, cx);
        } else if self.rows[ix].depth > 0 {
            let depth = self.rows[ix].depth;
            if let Some(parent) = (0..ix).rev().find(|&i| self.rows[i].depth < depth) {
                self.select_only(parent);
                self.scroll_to(parent, ScrollStrategy::Nearest);
                cx.notify();
            }
        }
    }

    // -- selection ----------------------------------------------------------

    pub fn index_of(&self, path: &Path) -> Option<usize> {
        self.rows.iter().position(|r| r.entry.path == path)
    }

    fn cursor_ix(&self) -> Option<usize> {
        self.cursor.as_ref().and_then(|p| self.index_of(p))
    }

    fn select_only(&mut self, ix: usize) {
        let path = self.rows[ix].entry.path.clone();
        self.selection.clear();
        self.selection.insert(path.clone());
        self.anchor = Some(path.clone());
        self.cursor = Some(path);
    }

    fn scroll_to(&self, ix: usize, strategy: ScrollStrategy) {
        if self.view == ViewMode::List {
            self.list_scroll.scroll_to_item(ix, strategy);
        }
    }

    pub fn click_row(&mut self, ix: usize, modifiers: Modifiers, cx: &mut Context<Self>) {
        if ix >= self.rows.len() {
            return;
        }
        let path = self.rows[ix].entry.path.clone();
        if modifiers.shift {
            let anchor_ix = self
                .anchor
                .as_ref()
                .and_then(|p| self.index_of(p))
                .unwrap_or(ix);
            let (lo, hi) = (anchor_ix.min(ix), anchor_ix.max(ix));
            self.selection = self.rows[lo..=hi]
                .iter()
                .map(|r| r.entry.path.clone())
                .collect();
            self.cursor = Some(path);
        } else if modifiers.platform {
            if !self.selection.remove(&path) {
                self.selection.insert(path.clone());
            }
            self.anchor = Some(path.clone());
            self.cursor = Some(path);
        } else {
            self.select_only(ix);
        }
        cx.notify();
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            self.selection.clear();
            self.anchor = None;
            self.cursor = None;
            cx.notify();
        }
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.selection = self.rows.iter().map(|r| r.entry.path.clone()).collect();
        self.anchor = self.rows.first().map(|r| r.entry.path.clone());
        self.cursor = self.rows.last().map(|r| r.entry.path.clone());
        cx.notify();
    }

    pub fn move_cursor(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let n = self.rows.len();
        if n == 0 {
            return;
        }
        let next = match self.cursor_ix() {
            None => {
                if delta > 0 {
                    0
                } else {
                    n - 1
                }
            }
            Some(c) => (c as isize + delta).clamp(0, n as isize - 1) as usize,
        };
        if extend {
            let anchor_ix = self
                .anchor
                .as_ref()
                .and_then(|p| self.index_of(p))
                .unwrap_or(next);
            let (lo, hi) = (anchor_ix.min(next), anchor_ix.max(next));
            self.selection = self.rows[lo..=hi]
                .iter()
                .map(|r| r.entry.path.clone())
                .collect();
            self.cursor = Some(self.rows[next].entry.path.clone());
        } else {
            self.select_only(next);
        }
        self.scroll_to(next, ScrollStrategy::Nearest);
        cx.notify();
    }

    // -- preview ------------------------------------------------------------

    /// Kick off a background recursive size computation for `path` if needed.
    pub fn request_folder_size(&mut self, path: &Path, cx: &mut Context<Self>) {
        if self.folder_sizes.contains_key(path) {
            return;
        }
        self.folder_sizes
            .insert(path.to_path_buf(), FolderSize::Computing);
        let path = path.to_path_buf();
        cx.spawn(async move |this, cx| {
            let target = path.clone();
            let (bytes, complete) = cx
                .background_spawn(async move { fsx::folder_size(&target, 300_000) })
                .await;
            this.update(cx, |this, cx| {
                this.folder_sizes
                    .insert(path, FolderSize::Done { bytes, complete });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // -- search -------------------------------------------------------------

    fn run_search(&mut self, cx: &mut Context<Self>) {
        self.search_gen += 1;
        let generation = self.search_gen;
        self.search_results.clear();
        if self.query.is_empty() {
            self.search_task = None;
            self.searching = false;
            self.rebuild_rows();
            cx.notify();
            return;
        }
        self.searching = true;
        self.rebuild_rows();
        cx.notify();

        let root = self.cwd.clone();
        let query = self.query.clone();
        let show_hidden = self.show_hidden;
        self.search_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            let found = cx
                .background_spawn(
                    async move { fsx::search(&root, &query, show_hidden, 2000, 200_000) },
                )
                .await;
            this.update(cx, |this, cx| {
                if this.search_gen != generation {
                    return;
                }
                let mut results: Vec<Arc<Entry>> = found.into_iter().map(Arc::new).collect();
                fsx::sort_entries(&mut results, this.sort_key, this.sort_asc);
                this.search_results = results;
                this.searching = false;
                this.rebuild_rows();
                cx.notify();
            })
            .ok();
        }));
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.search.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    }

    pub fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |state, cx| state.emplace("", cx));
        window.focus(&self.focus, cx);
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn build_sidebar(home: &Path) -> Vec<SidebarSection> {
    let item = |label: &str, path: PathBuf, icon: &'static str| SidebarItem {
        label: label.to_string(),
        path,
        icon,
    };

    let mut favorites = vec![item(
        "Applications",
        PathBuf::from("/Applications"),
        "applications",
    )];
    for (label, dir, icon) in [
        ("Desktop", "Desktop", "desktop"),
        ("Documents", "Documents", "document"),
        ("Downloads", "Downloads", "download"),
        ("Developer", "Developer", "developer"),
    ] {
        let path = home.join(dir);
        if path.is_dir() {
            favorites.push(item(label, path, icon));
        }
    }

    let mut locations = Vec::new();
    let icloud = home.join("Library/Mobile Documents/com~apple~CloudDocs");
    if icloud.is_dir() {
        locations.push(item("iCloud Drive", icloud, "cloud"));
    }
    let user = home
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Home".into());
    locations.push(item(&user, home.to_path_buf(), "home"));

    let mut boot_name = None;
    let mut volumes = Vec::new();
    if let Ok(read) = std::fs::read_dir("/Volumes") {
        for v in read.flatten() {
            let path = v.path();
            let name = v.file_name().to_string_lossy().into_owned();
            if std::fs::canonicalize(&path).is_ok_and(|p| p == Path::new("/")) {
                boot_name = Some(name);
            } else if path.is_dir() && !name.starts_with('.') {
                volumes.push(item(&name, path, "drive"));
            }
        }
    }
    locations.push(item(
        boot_name.as_deref().unwrap_or("Macintosh HD"),
        PathBuf::from("/"),
        "drive",
    ));
    volumes.sort_by(|a, b| fsx::natural_cmp(&a.label, &b.label));
    locations.extend(volumes);
    locations.push(item("Trash", home.join(".Trash"), "trash"));

    vec![
        SidebarSection {
            title: "Favorites",
            items: favorites,
        },
        SidebarSection {
            title: "Locations",
            items: locations,
        },
    ]
}
