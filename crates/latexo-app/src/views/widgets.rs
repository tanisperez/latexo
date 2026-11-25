//! The small building blocks every screen shares, from the design: the card, its header with the
//! icon tile, the big value, the chart well, the bar rows, the key and value rows and the legend.
//! Colors only from the palette, sizes only from `theme::metrics`.

use gpui_kit::assets::IconName;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::Palette;
use crate::theme::metrics::{
    BAR_HEIGHT, CARD_ICON_SIZE, CARD_PADDING, CHART_WELL_PADDING_X, CHART_WELL_PADDING_Y,
    RADIUS_LG, RADIUS_MD, RADIUS_XS, STAT_ROW_HEIGHT, SWATCH_SIZE, TEXT_ROW, TEXT_SM,
    VALUE_TEXT_SIZE,
};

/// A card: raised surface, border, large radius and padding.
pub fn card(palette: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .p(px(CARD_PADDING))
        .bg(palette.raised)
        .border_1()
        .border_color(palette.border)
        .rounded(px(RADIUS_LG))
}

/// The icon tile at the top left of a card.
pub fn icon_tile(icon: IconName, palette: &Palette) -> Div {
    div()
        .flex_none()
        .size(px(CARD_ICON_SIZE))
        .rounded(px(RADIUS_MD))
        .bg(palette.accent_subtle)
        .text_color(palette.accent_text)
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).with_size(px(16.0)))
}

/// A card's header: the icon tile, the title and whatever goes at the right.
pub fn card_header(
    icon: Option<IconName>,
    title: impl Into<SharedString>,
    palette: &Palette,
) -> Div {
    let row = h_flex().items_center().gap(px(10.0));
    let row = match icon {
        Some(icon) => row.child(icon_tile(icon, palette)),
        None => row,
    };
    row.child(
        div()
            .flex_1()
            .min_w_0()
            .text_size(px(13.0))
            .font_weight(FontWeight::SEMIBOLD)
            .truncate()
            .child(title.into()),
    )
}

/// The big value of a card and the line under it.
pub fn big_value(value: impl Into<SharedString>, size: f32) -> Div {
    div()
        .text_size(px(size))
        .line_height(px(size + 2.0))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .child(value.into())
}

/// The big value with its unit line beside it, as the detail screens show it.
pub fn value_line(
    value: impl Into<SharedString>,
    sub: impl Into<SharedString>,
    palette: &Palette,
) -> Div {
    h_flex()
        .items_baseline()
        .gap(px(10.0))
        .child(big_value(value, VALUE_TEXT_SIZE))
        .child(
            div()
                .text_size(px(TEXT_ROW))
                .text_color(palette.fg_muted)
                .truncate()
                .child(sub.into()),
        )
}

/// A muted line of small text.
pub fn muted(text: impl Into<SharedString>, palette: &Palette) -> Div {
    div()
        .text_size(px(TEXT_SM))
        .text_color(palette.fg_muted)
        .truncate()
        .child(text.into())
}

/// The sunken well a chart sits in.
pub fn chart_well(chart: impl IntoElement, palette: &Palette) -> Div {
    div()
        .w_full()
        .px(px(CHART_WELL_PADDING_X))
        .py(px(CHART_WELL_PADDING_Y))
        .rounded(px(RADIUS_MD))
        .bg(palette.surface)
        .child(chart)
}

/// A thin bar from 0 to 1.
pub fn bar(fraction: f32, color: Hsla, palette: &Palette) -> Div {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    div()
        .w_full()
        .h(px(BAR_HEIGHT))
        .rounded(px(BAR_HEIGHT / 2.0))
        .bg(palette.surface)
        .border_1()
        .border_color(palette.border)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(fraction))
                .rounded(px(BAR_HEIGHT / 2.0))
                .bg(color),
        )
}

/// A label and its value above a bar, as the lists of drives, sensors and engines.
pub fn bar_row(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    fraction: f32,
    color: Hsla,
    palette: &Palette,
) -> Div {
    v_flex()
        .gap(px(6.0))
        .min_w_0()
        .child(
            h_flex()
                .justify_between()
                .gap(px(8.0))
                .text_size(px(TEXT_ROW))
                .child(div().min_w_0().truncate().child(label.into()))
                .child(
                    div()
                        .flex_none()
                        .font_family(palette_mono())
                        .text_size(px(TEXT_SM))
                        .text_color(palette.fg_muted)
                        .child(value.into()),
                ),
        )
        .child(bar(fraction, color, palette))
}

/// The monospace family of the theme, for numbers.
pub fn palette_mono() -> SharedString {
    SharedString::new_static("Geist Mono")
}

/// Key and value rows split by lines, as the Details card.
pub fn stat_rows(rows: Vec<(String, String)>, palette: &Palette) -> Div {
    v_flex().children(rows.into_iter().enumerate().map(|(index, (key, value))| {
        h_flex()
            .h(px(STAT_ROW_HEIGHT))
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .when(index > 0, |row| {
                row.border_t_1().border_color(palette.border)
            })
            .child(
                div()
                    .flex_none()
                    .text_size(px(TEXT_ROW))
                    .text_color(palette.fg_muted)
                    .child(key),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_family(palette_mono())
                    .text_size(px(TEXT_SM))
                    .child(value),
            )
    }))
}

/// Name and value rows with the name in the normal color, as the lists of processes.
pub fn name_rows(rows: Vec<(String, String)>, palette: &Palette) -> Div {
    v_flex().children(rows.into_iter().enumerate().map(|(index, (key, value))| {
        h_flex()
            .h(px(STAT_ROW_HEIGHT))
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .when(index > 0, |row| {
                row.border_t_1().border_color(palette.border)
            })
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(TEXT_ROW))
                    .child(key),
            )
            .child(
                div()
                    .flex_none()
                    .font_family(palette_mono())
                    .text_size(px(TEXT_SM))
                    .child(value),
            )
    }))
}

/// A legend entry: a small swatch and its label.
pub fn legend_item(label: impl Into<SharedString>, color: Hsla, palette: &Palette) -> Div {
    h_flex()
        .items_center()
        .gap(px(6.0))
        .text_size(px(TEXT_SM))
        .text_color(palette.fg_muted)
        .child(
            div()
                .size(px(SWATCH_SIZE))
                .rounded(px(RADIUS_XS / 2.0))
                .bg(color),
        )
        .child(label.into())
}

/// A small heading inside a card.
pub fn card_title(title: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .mb(px(6.0))
        .child(title.into())
}

/// A pill with a dot: the badges of the headers and of Settings.
pub fn badge(label: impl Into<SharedString>, fg: Hsla, bg: Hsla) -> Div {
    h_flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(22.0))
        .px(px(9.0))
        .rounded(px(11.0))
        .bg(bg)
        .text_color(fg)
        .text_size(px(TEXT_SM))
        .font_weight(FontWeight::MEDIUM)
        .child(div().size(px(6.0)).rounded_full().bg(fg))
        .child(label.into())
}

/// A note inside a card, as the fan control note of the design.
pub fn note(text: impl Into<SharedString>, fg: Hsla, bg: Hsla) -> Div {
    h_flex()
        .items_start()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(RADIUS_MD))
        .bg(bg)
        .text_color(fg)
        .text_size(px(TEXT_SM))
        .child(Icon::new(IconName::Info).with_size(px(14.0)).flex_none())
        .child(div().flex_1().min_w_0().child(text.into()))
}

/// An empty card body: a muted line, centered.
pub fn empty_line(text: impl Into<SharedString>, palette: &Palette) -> Div {
    div()
        .w_full()
        .py(px(24.0))
        .flex()
        .justify_center()
        .text_size(px(TEXT_ROW))
        .text_color(palette.fg_muted)
        .child(text.into())
}
