//! Finder-style file and folder glyphs, drawn from plain divs and gradients.

use gpui::{Div, Rgba, Styled, div, linear_color_stop, linear_gradient, prelude::*, px, rgb, svg};

use crate::{fsx::IconKind, theme::Theme};

/// Monochrome SVG from the embedded asset set.
pub fn glyph(name: &str, size: f32, color: Rgba) -> gpui::Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

pub fn entry_icon(kind: IconKind, size: f32, theme: &Theme) -> Div {
    match kind {
        IconKind::Folder => folder(size, theme),
        IconKind::App => app(size),
        other => page(other, size, theme),
    }
}

fn folder(size: f32, theme: &Theme) -> Div {
    let w = size;
    let h = size * 0.8;
    let radius = size * 0.1;
    div()
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(size))
        .child(
            // Back panel with tab.
            div()
                .absolute()
                .top(px(size * 0.1))
                .left_0()
                .w(px(w * 0.44))
                .h(px(size * 0.2))
                .rounded_t(px(radius))
                .bg(theme.folder_back),
        )
        .child(
            div()
                .absolute()
                .top(px(size * 0.18))
                .left_0()
                .w(px(w))
                .h(px(h - size * 0.1))
                .rounded(px(radius))
                .bg(theme.folder_back),
        )
        .child(
            // Paper sheet peeking out.
            div()
                .absolute()
                .top(px(size * 0.27))
                .left(px(w * 0.06))
                .w(px(w * 0.88))
                .h(px(size * 0.3))
                .rounded_t(px(radius * 0.6))
                .bg(rgb(0xffffff)),
        )
        .child(
            // Front panel.
            div()
                .absolute()
                .top(px(size * 0.34))
                .left_0()
                .w(px(w))
                .h(px(size * 0.56))
                .rounded(px(radius))
                .bg(linear_gradient(
                    180.0,
                    linear_color_stop(theme.folder_front_top, 0.0),
                    linear_color_stop(theme.folder_front_bottom, 1.0),
                )),
        )
}

fn app(size: f32) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(size * 0.24))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgb(0x6aa8ff), 0.0),
            linear_color_stop(rgb(0x2f6be0), 1.0),
        ))
        .child(glyph("applications", size * 0.62, rgb(0xffffff)))
}

fn page(kind: IconKind, size: f32, theme: &Theme) -> Div {
    let (bg, glyph_name, glyph_color): (Rgba, Option<&str>, Rgba) = match kind {
        IconKind::Data => (rgb(0x3d6fdc), Some("glyph-braces"), rgb(0xffffff)),
        IconKind::Image => (rgb(0xe9eef6), Some("glyph-image"), rgb(0x7b97c2)),
        IconKind::Archive => (rgb(0xc6a366), Some("glyph-archive"), rgb(0xffffff)),
        IconKind::Media => (rgb(0x3a3a3f), Some("glyph-play"), rgb(0xffffff)),
        IconKind::Text => (rgb(0xffffff), Some("glyph-text"), rgb(0xa5a5ad)),
        _ => (rgb(0xffffff), None, rgb(0xffffff)),
    };
    let border = if theme.dark {
        rgb(0x55555b)
    } else {
        rgb(0xc9c9ce)
    };
    let w = size * 0.74;
    div()
        .flex_none()
        .w(px(size))
        .h(px(size))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(w))
                .h(px(size * 0.94))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(size * 0.1))
                .border_1()
                .border_color(border)
                .bg(bg)
                .children(glyph_name.map(|name| glyph(name, w * 0.52, glyph_color))),
        )
}
