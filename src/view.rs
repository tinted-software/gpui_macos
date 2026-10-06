//! Finder rendering: sidebar, toolbar, list / icon views, preview pane.

use std::{ops::Range, path::Path, sync::Arc};

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ObjectFit, ParentElement, Render, Rgba, Stateful, Styled, Window, div, img, prelude::*, px,
    rgba, uniform_list,
};
use gpui_ce_elements::editable_text::text_input;
use palette::IntoColor;

use crate::{
    finder::{Finder, Row, ViewMode},
    fsx::{self, Entry, FolderSize, IconKind, SortKey},
    icons::{entry_icon, glyph},
    theme::Theme,
};

const SIDEBAR_W: f32 = 210.0;
const PREVIEW_W: f32 = 240.0;
const ROW_H: f32 = 26.0;
const HEADER_H: f32 = 26.0;
const DATE_W: f32 = 150.0;
const SIZE_W: f32 = 90.0;
const KIND_W: f32 = 170.0;
const CELL_W: f32 = 104.0;
const CELL_H: f32 = 108.0;

impl Render for Finder {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = Theme::for_window(window);
        let theme = self.theme;

        if self.applied_title.as_deref() != Some(self.cwd.as_path()) {
            let title = self
                .cwd
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".into());
            window.set_window_title(&title);
            self.applied_title = Some(self.cwd.clone());
        }

        let content_w = f32::from(window.viewport_size().width) - SIDEBAR_W - PREVIEW_W - 24.0;
        self.icon_cols = ((content_w / CELL_W).floor() as usize).max(1);

        // Kick off folder-size computation for the previewed folder.
        if let Some(subject) = self.preview_subject()
            && subject.is_dir
        {
            let path = subject.path.clone();
            self.request_folder_size(&path, cx);
        }

        div()
            .id("finder")
            .key_context("Finder")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &crate::finder::GoBack, _, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &crate::finder::GoForward, _, cx| this.go_forward(cx)))
            .on_action(cx.listener(|this, _: &crate::finder::GoUp, _, cx| this.go_up(cx)))
            .on_action(
                cx.listener(|this, _: &crate::finder::OpenSelected, _, cx| this.open_selected(cx)),
            )
            .on_action(cx.listener(|this, _: &crate::finder::SelectNext, _, cx| {
                let step = if this.view == ViewMode::Icons {
                    this.icon_cols as isize
                } else {
                    1
                };
                this.move_cursor(step, false, cx)
            }))
            .on_action(
                cx.listener(|this, _: &crate::finder::SelectPrevious, _, cx| {
                    let step = if this.view == ViewMode::Icons {
                        this.icon_cols as isize
                    } else {
                        1
                    };
                    this.move_cursor(-step, false, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::finder::SelectRight, _, cx| {
                    this.expand_or_descend(cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::finder::SelectLeft, _, cx| {
                    this.collapse_or_ascend(cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::finder::ExpandOrDescend, _, cx| {
                    this.expand_or_descend(cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::finder::CollapseOrAscend, _, cx| {
                    this.collapse_or_ascend(cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::finder::SelectEverything, _, cx| this.select_all(cx)),
            )
            .on_action(
                cx.listener(|this, _: &crate::finder::ToggleHidden, _, cx| this.toggle_hidden(cx)),
            )
            .on_action(cx.listener(|this, _: &crate::finder::Refresh, _, cx| this.refresh(cx)))
            .on_action(cx.listener(|this, _: &crate::finder::ShowList, _, cx| {
                this.set_view(ViewMode::List, cx)
            }))
            .on_action(cx.listener(|this, _: &crate::finder::ShowIcons, _, cx| {
                this.set_view(ViewMode::Icons, cx)
            }))
            .on_action(
                cx.listener(|this, _: &crate::finder::FocusSearch, window, cx| {
                    this.focus_search(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::finder::ClearSearch, window, cx| {
                    this.clear_search(window, cx)
                }),
            )
            .size_full()
            .bg(theme.window_bg)
            .flex()
            .flex_row()
            .font_family(".SystemUIFont")
            .text_size(px(13.0))
            .text_color(theme.text)
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(theme.content_bg)
                    .child(self.render_toolbar(cx))
                    .child(self.render_body(cx)),
            )
            .child(self.render_preview(cx))
    }
}

// ---------------------------------------------------------------------------
// Small controls
// ---------------------------------------------------------------------------

fn icon_button(
    id: &'static str,
    icon: &str,
    enabled: bool,
    active: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let color = if enabled {
        theme.text
    } else {
        theme.text_tertiary
    };
    let hover = theme.hover;
    let mut button = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(34.0))
        .h(px(28.0))
        .rounded(px(8.0))
        .child(glyph(icon, 16.0, color));
    if active {
        button = button.bg(theme.sidebar_selected);
    } else if enabled {
        button = button.hover(move |s| s.bg(hover));
    }
    button
}

fn transparent() -> Rgba {
    rgba(0x00000000)
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

impl Finder {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut panel = div()
            .flex()
            .flex_col()
            .size_full()
            .rounded(px(16.0))
            .bg(theme.sidebar_bg)
            .pt(px(48.0))
            .px(px(10.0))
            .pb(px(10.0))
            .gap(px(14.0))
            .overflow_hidden();

        for (section_ix, section) in self.sidebar.iter().enumerate() {
            let mut group = div().flex().flex_col().gap(px(1.0)).child(
                div()
                    .px(px(8.0))
                    .pb(px(3.0))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_secondary)
                    .child(section.title),
            );
            for (ix, item) in section.items.iter().enumerate() {
                let selected = item.path == self.cwd;
                let path = item.path.clone();
                let hover = theme.hover;
                group = group.child(
                    div()
                        .id(("sidebar", section_ix * 100 + ix))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .h(px(28.0))
                        .px(px(8.0))
                        .rounded(px(8.0))
                        .cursor_pointer()
                        .when(selected, |d| d.bg(theme.sidebar_selected))
                        .when(!selected, |d| d.hover(move |s| s.bg(hover)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            window.focus(&this.focus, cx);
                            this.navigate(path.clone(), cx);
                        }))
                        .child(glyph(item.icon, 16.0, theme.accent))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(item.label.clone()),
                        ),
                );
            }
            panel = panel.child(group);
        }

        div()
            .w(px(SIDEBAR_W))
            .h_full()
            .flex_none()
            .p(px(8.0))
            .child(panel)
    }

    // -----------------------------------------------------------------------
    // Toolbar
    // -----------------------------------------------------------------------

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let back = self.can_go_back();
        let forward = self.can_go_forward();

        let title = if self.query.is_empty() {
            self.cwd.display().to_string()
        } else {
            format!("Searching “{}” in {}", self.query, self.cwd.display())
        };

        div()
            .flex_none()
            .h(px(52.0))
            .w_full()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(12.0))
            .border_b_1()
            .border_color(theme.divider)
            .child(
                icon_button("back", "chevron-left", back, false, &theme)
                    .on_click(cx.listener(|this, _, _, cx| this.go_back(cx))),
            )
            .child(
                icon_button("forward", "chevron-right", forward, false, &theme)
                    .on_click(cx.listener(|this, _, _, cx| this.go_forward(cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(8.0))
                    .truncate()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(2.0))
                    .p(px(2.0))
                    .rounded(px(10.0))
                    .bg(theme.field_bg)
                    .child(
                        icon_button(
                            "view-icons",
                            "view-icons",
                            true,
                            self.view == ViewMode::Icons,
                            &theme,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.set_view(ViewMode::Icons, cx))),
                    )
                    .child(
                        icon_button(
                            "view-list",
                            "view-list",
                            true,
                            self.view == ViewMode::List,
                            &theme,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.set_view(ViewMode::List, cx))),
                    ),
            )
            .child(
                div()
                    .key_context("SearchField")
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(6.0))
                    .ml(px(8.0))
                    .w(px(190.0))
                    .h(px(28.0))
                    .px(px(10.0))
                    .rounded(px(14.0))
                    .bg(theme.field_bg)
                    .child(glyph("search", 14.0, theme.text_secondary))
                    .child(
                        text_input("search-field")
                            .state(self.search.downgrade())
                            .placeholder("Search")
                            .placeholder_color(theme.text_secondary)
                            .flex_1()
                            .min_w_0()
                            .min_h_auto()
                            .whitespace_nowrap()
                            .text_size(px(13.0))
                            .text_color(theme.text)
                            .caret_color(theme.accent.into_color())
                            .caret_blink_interval_500ms(),
                    )
                    .when(!self.query.is_empty(), |d| {
                        d.child(
                            div()
                                .id("clear-search")
                                .cursor_pointer()
                                .child(glyph("x-circle", 14.0, theme.text_tertiary))
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.clear_search(window, cx)
                                    }),
                                ),
                        )
                    }),
            )
    }

    // -----------------------------------------------------------------------
    // Body
    // -----------------------------------------------------------------------

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let message = if let Some(err) = self.cwd_error() {
            Some(err.to_string())
        } else if self.is_empty_folder() {
            Some("This folder is empty.".to_string())
        } else if !self.query.is_empty() && self.rows.is_empty() {
            Some(if self.searching {
                "Searching…".to_string()
            } else {
                "No results.".to_string()
            })
        } else {
            None
        };

        let content: AnyElement = if let Some(message) = message {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.0))
                .px(px(32.0))
                .text_center()
                .text_color(theme.text_secondary)
                .child(message)
                .when(self.cwd_denied(), |d| {
                    d.child(
                        div()
                            .id("open-privacy")
                            .px(px(12.0))
                            .py(px(5.0))
                            .rounded(px(7.0))
                            .bg(theme.accent)
                            .text_color(theme.selection_text)
                            .cursor_pointer()
                            .child("Open Privacy Settings")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.open_url(
                                    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
                                )
                            })),
                    )
                })
                .into_any_element()
        } else if self.view == ViewMode::List {
            uniform_list(
                "rows",
                self.rows.len(),
                cx.processor(|this, range: Range<usize>, _window, cx| {
                    range.map(|ix| this.render_row(ix, cx)).collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.list_scroll)
            .flex_1()
            .into_any_element()
        } else {
            self.render_icon_grid(cx).into_any_element()
        };

        let mut body = div().flex_1().min_h_0().flex().flex_col();
        if self.view == ViewMode::List {
            body = body.child(self.render_header(cx));
        }
        body.child(
            div()
                .id("list-area")
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .on_click(cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus, cx);
                    this.clear_selection(cx);
                }))
                .child(content),
        )
        .into_any_element()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let cell =
            |label: &'static str, key: SortKey, width: Option<f32>, cx: &mut Context<Self>| {
                let active = self.sort_key == key;
                let mut cell = div()
                    .id(label)
                    .flex()
                    .items_center()
                    .h_full()
                    .px(px(8.0))
                    .gap(px(4.0))
                    .text_size(px(11.0))
                    .text_color(if active {
                        theme.text
                    } else {
                        theme.text_secondary
                    })
                    .child(label)
                    .when(active, |d| {
                        d.child(glyph(
                            if self.sort_asc {
                                "chevron-up"
                            } else {
                                "chevron-down"
                            },
                            10.0,
                            theme.text_secondary,
                        ))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.sort_by(key, cx)));
                cell = match width {
                    Some(w) => cell
                        .w(px(w))
                        .flex_none()
                        .border_l_1()
                        .border_color(theme.divider),
                    None => cell.flex_1().min_w_0(),
                };
                cell
            };

        div()
            .flex_none()
            .h(px(HEADER_H))
            .w_full()
            .px(px(8.0))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(theme.divider)
            .child(div().w(px(8.0)).flex_none())
            .child(cell("Name", SortKey::Name, None, cx))
            .child(cell("Date Modified", SortKey::Modified, Some(DATE_W), cx))
            .child(cell("Size", SortKey::Size, Some(SIZE_W), cx))
            .child(cell("Kind", SortKey::Kind, Some(KIND_W), cx))
    }

    fn render_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Row { entry, depth } = &self.rows[ix];
        let selected = self.selection.contains(&entry.path);
        let expanded = self.is_expanded(&entry.path);
        let searching = !self.query.is_empty();

        let (primary, secondary) = if selected {
            (theme.selection_text, rgba(0xffffffcc))
        } else {
            (theme.text, theme.text_secondary)
        };
        let bg = if selected {
            theme.selection
        } else if ix % 2 == 1 {
            theme.row_alt
        } else {
            transparent()
        };

        let chevron: AnyElement = if entry.is_dir && !searching {
            let path = entry.path.clone();
            div()
                .id(("disclosure", ix))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .w(px(18.0))
                .h_full()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_expanded(&path, cx);
                }))
                .child(glyph(
                    if expanded {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    10.0,
                    secondary,
                ))
                .into_any_element()
        } else {
            div().w(px(18.0)).flex_none().into_any_element()
        };

        let location_hint = if searching {
            entry
                .path
                .parent()
                .and_then(|p| p.strip_prefix(&self.cwd).ok())
                .map(|p| p.display().to_string())
                .filter(|p| !p.is_empty())
        } else {
            None
        };

        let click_entry = entry.clone();
        div()
            .id(("row", ix))
            .flex_none()
            .w_full()
            .h(px(ROW_H))
            .px(px(8.0))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                cx.stop_propagation();
                if ev.is_right_click() {
                    return;
                }
                window.focus(&this.focus, cx);
                this.click_row(ix, ev.modifiers(), cx);
                if ev.click_count() >= 2 {
                    this.open_entry(&click_entry, cx);
                }
            }))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .rounded(px(7.0))
                    .bg(bg)
                    .text_color(primary)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .pl(px(*depth as f32 * 20.0 + 4.0))
                            .child(chevron)
                            .child(entry_icon(entry.icon, 18.0, &theme))
                            .child(
                                div()
                                    .flex_none()
                                    .max_w_full()
                                    .truncate()
                                    .child(entry.name.clone()),
                            )
                            .children(location_hint.map(|hint| {
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(11.0))
                                    .text_color(secondary)
                                    .child(hint)
                            })),
                    )
                    .child(
                        div()
                            .w(px(DATE_W))
                            .flex_none()
                            .px(px(8.0))
                            .truncate()
                            .text_size(px(12.0))
                            .text_color(secondary)
                            .child(fsx::format_date_short(entry.modified)),
                    )
                    .child(
                        div()
                            .w(px(SIZE_W))
                            .flex_none()
                            .px(px(8.0))
                            .flex()
                            .justify_end()
                            .text_size(px(12.0))
                            .text_color(secondary)
                            .child(match entry.size {
                                Some(size) => fsx::format_size(size),
                                None => "--".to_string(),
                            }),
                    )
                    .child(
                        div()
                            .w(px(KIND_W))
                            .flex_none()
                            .px(px(8.0))
                            .truncate()
                            .text_size(px(12.0))
                            .text_color(secondary)
                            .child(entry.kind.clone()),
                    ),
            )
            .into_any_element()
    }

    fn render_icon_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let cells: Vec<AnyElement> = self
            .rows
            .iter()
            .enumerate()
            .map(|(ix, row)| {
                let entry = row.entry.clone();
                let selected = self.selection.contains(&entry.path);
                let open_entry = entry.clone();
                div()
                    .id(("cell", ix))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(5.0))
                    .flex_none()
                    .w(px(CELL_W))
                    .h(px(CELL_H))
                    .pt(px(8.0))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        if ev.is_right_click() {
                            return;
                        }
                        window.focus(&this.focus, cx);
                        this.click_row(ix, ev.modifiers(), cx);
                        if ev.click_count() >= 2 {
                            this.open_entry(&open_entry, cx);
                        }
                    }))
                    .child(
                        div()
                            .p(px(4.0))
                            .rounded(px(8.0))
                            .when(selected, |d| d.bg(theme.sidebar_selected))
                            .child(entry_icon(entry.icon, 56.0, &theme)),
                    )
                    .child(
                        div()
                            .max_w(px(CELL_W - 8.0))
                            .px(px(5.0))
                            .py(px(1.0))
                            .rounded(px(5.0))
                            .text_size(px(12.0))
                            .text_center()
                            .line_clamp(2)
                            .when(selected, |d| {
                                d.bg(theme.selection).text_color(theme.selection_text)
                            })
                            .child(entry.name.clone()),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .id("icon-grid")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .content_start()
                    .p(px(12.0))
                    .children(cells),
            )
    }

    // -----------------------------------------------------------------------
    // Preview pane
    // -----------------------------------------------------------------------

    /// The entry shown in the preview pane: the selection if it is a single
    /// item, otherwise (nothing selected) the current folder.
    pub fn preview_subject(&self) -> Option<Arc<Entry>> {
        match self.selection.len() {
            0 => Entry::from_path(&self.cwd).map(Arc::new),
            1 => self
                .rows
                .iter()
                .find(|r| self.selection.contains(&r.entry.path))
                .map(|r| r.entry.clone()),
            _ => None,
        }
    }

    fn folder_size_text(&self, path: &Path) -> Option<String> {
        match self.folder_sizes.get(path)? {
            FolderSize::Computing => None,
            FolderSize::Done { bytes, complete } => Some(format!(
                "{}{}",
                fsx::format_size(*bytes),
                if *complete { "" } else { "+" }
            )),
        }
    }

    fn render_preview(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let pane = div()
            .w(px(PREVIEW_W))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme.preview_bg)
            .border_l_1()
            .border_color(theme.divider)
            .px(px(16.0))
            .pb(px(16.0));

        let Some(entry) = self.preview_subject() else {
            let count = self.selection.len();
            return pane
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{count} items")),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.text_secondary)
                        .child("Multiple items selected"),
                );
        };

        let size_text = if entry.is_dir {
            self.folder_size_text(&entry.path)
        } else {
            entry.size.map(fsx::format_size)
        };
        let subtitle = match size_text {
            Some(size) => format!("{} – {}", entry.kind, size),
            None => entry.kind.clone(),
        };

        let hero: AnyElement = if entry.is_image {
            img(entry.path.clone())
                .max_w(px(PREVIEW_W - 32.0))
                .max_h(px(190.0))
                .object_fit(ObjectFit::Contain)
                .rounded(px(4.0))
                .into_any_element()
        } else {
            let size = if entry.icon == IconKind::Folder {
                150.0
            } else {
                120.0
            };
            entry_icon(entry.icon, size, &theme).into_any_element()
        };

        let info_row = |label: &'static str, value: String| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .py(px(3.0))
                .text_size(px(11.0))
                .child(
                    div()
                        .flex_none()
                        .text_color(theme.text_secondary)
                        .child(label),
                )
                .child(div().min_w_0().truncate().child(value))
        };
        let section_title = |label: &'static str| {
            div()
                .pt(px(10.0))
                .pb(px(4.0))
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label)
        };
        let rule = || div().h(px(1.0)).w_full().bg(theme.divider);

        pane.child(
            div()
                .h(px(250.0))
                .flex_none()
                .flex()
                .items_end()
                .justify_center()
                .pb(px(18.0))
                .child(hero),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(13.0))
                        .child(entry.name.clone()),
                )
                .child(
                    div()
                        .pb(px(6.0))
                        .text_size(px(11.0))
                        .text_color(theme.text_secondary)
                        .child(subtitle),
                )
                .child(rule())
                .child(section_title("Information"))
                .child(info_row("Created", fsx::format_date_long(entry.created)))
                .child(rule())
                .child(info_row("Modified", fsx::format_date_long(entry.modified)))
                .child(rule())
                .child(info_row(
                    "Last opened",
                    fsx::format_date_relative(entry.accessed),
                )),
        )
    }
}
