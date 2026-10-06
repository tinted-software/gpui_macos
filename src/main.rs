mod assets;
mod finder;
mod fsx;
mod icons;
mod theme;
mod view;

use gpui::{
    App, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions, point,
    prelude::*, px, size,
};
use gpui_ce_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;

use finder::{
    ClearSearch, CloseWindow, CollapseOrAscend, ExpandOrDescend, Finder, FocusSearch, GoBack,
    GoForward, GoUp, OpenSelected, Quit, Refresh, SelectEverything, SelectLeft, SelectNext,
    SelectPrevious, SelectRight, ShowIcons, ShowList, ToggleHidden,
};

fn main() {
    env_logger::init();

    application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(|_: &CloseWindow, cx| {
                if let Some(window) = cx.active_window() {
                    window
                        .update(cx, |_, window, _| window.remove_window())
                        .ok();
                }
            });

            cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-w", CloseWindow, None),
                KeyBinding::new("cmd-[", GoBack, Some("Finder")),
                KeyBinding::new("cmd-]", GoForward, Some("Finder")),
                KeyBinding::new("cmd-up", GoUp, Some("Finder")),
                KeyBinding::new("cmd-down", OpenSelected, Some("Finder")),
                KeyBinding::new("enter", OpenSelected, Some("Finder")),
                KeyBinding::new("down", SelectNext, Some("Finder")),
                KeyBinding::new("up", SelectPrevious, Some("Finder")),
                KeyBinding::new("right", SelectRight, Some("Finder")),
                KeyBinding::new("left", SelectLeft, Some("Finder")),
                KeyBinding::new("cmd-right", ExpandOrDescend, Some("Finder")),
                KeyBinding::new("cmd-left", CollapseOrAscend, Some("Finder")),
                KeyBinding::new("cmd-a", SelectEverything, Some("Finder")),
                KeyBinding::new("cmd-shift-.", ToggleHidden, Some("Finder")),
                KeyBinding::new("cmd-r", Refresh, Some("Finder")),
                KeyBinding::new("cmd-1", ShowIcons, Some("Finder")),
                KeyBinding::new("cmd-2", ShowList, Some("Finder")),
                KeyBinding::new("cmd-f", FocusSearch, Some("Finder")),
                KeyBinding::new("escape", ClearSearch, Some("SearchField")),
            ]);

            cx.set_menus(vec![
                Menu {
                    name: "Finder".into(),
                    items: vec![MenuItem::action("Quit Finder", Quit)],
                    disabled: false,
                },
                Menu {
                    name: "View".into(),
                    items: vec![
                        MenuItem::action("as Icons", ShowIcons),
                        MenuItem::action("as List", ShowList),
                        MenuItem::separator(),
                        MenuItem::action("Show Hidden Files", ToggleHidden),
                        MenuItem::action("Refresh", Refresh),
                    ],
                    disabled: false,
                },
                Menu {
                    name: "Go".into(),
                    items: vec![
                        MenuItem::action("Back", GoBack),
                        MenuItem::action("Forward", GoForward),
                        MenuItem::action("Enclosing Folder", GoUp),
                    ],
                    disabled: false,
                },
            ]);

            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = Bounds::centered(None, size(px(1180.0), px(720.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Finder".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(20.0), px(20.0))),
                    }),
                    window_min_size: Some(size(px(720.0), px(400.0))),
                    ..WindowOptions::new()
                },
                |window, cx| cx.new(|cx| Finder::new(window, cx)),
            )
            .expect("failed to open window");

            cx.activate(true);
        });
}
