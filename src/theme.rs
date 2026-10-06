use gpui::{Rgba, Window, WindowAppearance, rgb, rgba};

/// Finder palette; picked from the window appearance each frame.
#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    /// Window backdrop behind the floating sidebar panel.
    pub window_bg: Rgba,
    pub sidebar_bg: Rgba,
    pub sidebar_selected: Rgba,
    pub content_bg: Rgba,
    pub preview_bg: Rgba,
    pub row_alt: Rgba,
    pub selection: Rgba,
    pub selection_text: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub text_tertiary: Rgba,
    pub divider: Rgba,
    pub accent: Rgba,
    pub hover: Rgba,
    pub field_bg: Rgba,
    pub folder_back: Rgba,
    pub folder_front_top: Rgba,
    pub folder_front_bottom: Rgba,
}

impl Theme {
    pub fn for_window(window: &Window) -> Self {
        match window.appearance() {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            window_bg: rgb(0xe3e3e6),
            sidebar_bg: rgb(0xf0f0f2),
            sidebar_selected: rgba(0x0000001c),
            content_bg: rgb(0xffffff),
            preview_bg: rgb(0xf3f3f4),
            row_alt: rgb(0xf5f5f6),
            selection: rgb(0x2b5fd3),
            selection_text: rgb(0xffffff),
            text: rgb(0x1d1d1f),
            text_secondary: rgb(0x6e6e73),
            text_tertiary: rgb(0x9a9aa0),
            divider: rgba(0x00000018),
            accent: rgb(0x3478f6),
            hover: rgba(0x0000000d),
            field_bg: rgba(0x0000000f),
            folder_back: rgb(0x4ea3e8),
            folder_front_top: rgb(0x8ccbf5),
            folder_front_bottom: rgb(0x58b0ee),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            window_bg: rgb(0x19191b),
            sidebar_bg: rgb(0x2a2a2d),
            sidebar_selected: rgba(0xffffff22),
            content_bg: rgb(0x1e1e20),
            preview_bg: rgb(0x2a2a2c),
            row_alt: rgb(0x252527),
            selection: rgb(0x2b5fd3),
            selection_text: rgb(0xffffff),
            text: rgb(0xf2f2f4),
            text_secondary: rgb(0xa0a0a8),
            text_tertiary: rgb(0x75757c),
            divider: rgba(0xffffff1a),
            accent: rgb(0x4d8ff7),
            hover: rgba(0xffffff12),
            field_bg: rgba(0xffffff18),
            folder_back: rgb(0x3a86c8),
            folder_front_top: rgb(0x72b8ea),
            folder_front_bottom: rgb(0x4a9ae0),
        }
    }
}
