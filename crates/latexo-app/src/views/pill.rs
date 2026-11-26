//! The floating pill of the design: a small window always above the others with the main live
//! values (CPU, GPU, memory, power, temperature). On Wayland it is a layer shell surface anchored
//! at the top of the screen, which KDE, sway and Hyprland keep above every window (GNOME has no
//! layer shell, so it cannot open there); on Windows and X11 it is a pop-up window, always on
//! top, that the mouse drags. A double click brings the main window to the front.

use gpui_kit::component::{WindowExt as _, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::state::format;
use crate::theme::PaletteExt;
use crate::theme::metrics::{PILL_HEIGHT, PILL_WIDTH, TEXT_SM};

use super::logo::logo;
use super::root::AppView;
use super::widgets::palette_mono;

/// The pill's view: it reads the main view's monitor and redraws when it changes.
pub struct PillView {
    app: Entity<AppView>,
    main: AnyWindowHandle,
    _observe: Subscription,
}

impl PillView {
    fn new(app: Entity<AppView>, main: AnyWindowHandle, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&app, |_, _, cx| cx.notify());
        Self {
            app,
            main,
            _observe: observe,
        }
    }

    /// The label and value of each item, in the order of the design.
    fn items(&self, cx: &App) -> Vec<(String, String)> {
        let view = self.app.read(cx);
        let Some(sample) = view.monitor.latest.as_deref() else {
            return Vec::new();
        };
        let unit = view.settings.temperature_unit.into();
        let mut items = vec![(
            t!("nav.cpu").into_owned(),
            format::percent(sample.cpu.usage, 0),
        )];
        if let Some(usage) = sample.gpus.first().and_then(|gpu| gpu.usage) {
            items.push((t!("nav.gpu").into_owned(), format::percent(usage, 0)));
        }
        items.push((
            t!("pill.memory").into_owned(),
            format::percent(sample.memory.usage(), 0),
        ));
        if let Some(watts) = sample.power.measured_total() {
            items.push((t!("pill.power").into_owned(), format::watts(watts)));
        }
        if let Some(celsius) = sample.cpu_temperature() {
            items.push((
                t!("pill.temperature").into_owned(),
                format::temperature(celsius, unit),
            ));
        }
        items
    }
}

impl Render for PillView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.palette();
        let items = self.items(cx);
        let main = self.main;
        div()
            .id("pill")
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                h_flex()
                    .h(px(PILL_HEIGHT))
                    .px(px(14.0))
                    .gap(px(14.0))
                    .items_center()
                    .rounded(px(PILL_HEIGHT / 2.0))
                    .bg(palette.overlay)
                    .border_1()
                    .border_color(palette.border_strong)
                    .text_color(palette.fg)
                    .child(logo(20.0))
                    .children(items.into_iter().map(|(label, value)| {
                        h_flex()
                            .items_baseline()
                            .gap(px(6.0))
                            .whitespace_nowrap()
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(palette.fg_muted)
                                    .child(label),
                            )
                            .child(
                                div()
                                    .font_family(palette_mono())
                                    .text_size(px(TEXT_SM + 0.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(value),
                            )
                    })),
            )
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                if event.click_count >= 2 {
                    let _ = main.update(cx, |_, window, _| window.activate_window());
                } else {
                    // A pop-up window moves with the mouse; a layer shell surface stays anchored.
                    window.start_window_move();
                }
            })
    }
}

impl AppView {
    /// Opens or closes the floating pill. It opens after the current update (its first frame
    /// reads this view); when the desktop cannot show it, the setting goes back off with a
    /// notification.
    pub(crate) fn set_pill(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !on {
            if let Some(pill) = self.pill.take() {
                let _ = pill.update(cx, |_, window, _| window.remove_window());
                log::info!("floating pill closed");
            }
            return;
        }
        if self.pill.is_some() {
            return;
        }
        let app = cx.entity();
        let main = window.window_handle();
        let options = pill_options(cx);
        let theme = self.settings.theme;
        cx.spawn_in(window, async move |view, cx| {
            let opened = cx.update(|_, cx| {
                cx.open_window(options, move |window, cx| {
                    telaio_ui::ThemeChoice::from(theme).apply(Some(window), cx);
                    cx.new(|cx| PillView::new(app, main, cx))
                })
            });
            let _ = view.update_in(cx, |view, window, cx| match opened {
                Ok(Ok(handle)) => {
                    log::info!("floating pill opened");
                    view.pill = Some(handle.into());
                }
                Ok(Err(error)) | Err(error) => {
                    log::warn!("could not open the floating pill: {error}");
                    window.push_notification(
                        gpui_kit::component::notification::Notification::error(
                            t!("settings.pill_failed", error = error.to_string()).into_owned(),
                        ),
                        cx,
                    );
                    view.change(window, cx, |settings| settings.floating_pill = false);
                }
            });
        })
        .detach();
    }

    /// Closes the pill with the main window.
    pub fn close_pill(&mut self, cx: &mut App) {
        if let Some(pill) = self.pill.take() {
            let _ = pill.update(cx, |_, window, _| window.remove_window());
        }
    }
}

/// Where and how the pill opens: a layer shell surface on Wayland, a pop-up at the top center of
/// the primary display elsewhere.
fn pill_options(cx: &App) -> WindowOptions {
    let size = size(px(PILL_WIDTH), px(PILL_HEIGHT + 16.0));
    let origin = cx
        .primary_display()
        .map(|display| {
            let area = display.visible_bounds();
            point(
                area.origin.x + (area.size.width - size.width) / 2.0,
                area.origin.y + px(12.0),
            )
        })
        .unwrap_or_else(|| point(px(0.0), px(0.0)));
    WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, size))),
        focus: false,
        show: true,
        kind: pill_kind(cx),
        is_movable: true,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: Some(format!("{}.pill", latexo_cli::IDENTITY.app_id)),
        ..WindowOptions::default()
    }
}

#[cfg(target_os = "linux")]
fn pill_kind(cx: &App) -> WindowKind {
    use gpui_kit::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
    if cx.compositor_name() == "Wayland" {
        WindowKind::LayerShell(LayerShellOptions {
            namespace: "latexo-pill".to_string(),
            layer: Layer::Overlay,
            anchor: Anchor::TOP,
            exclusive_zone: None,
            exclusive_edge: None,
            margin: Some((px(8.0), px(0.0), px(0.0), px(0.0))),
            keyboard_interactivity: KeyboardInteractivity::None,
        })
    } else {
        WindowKind::PopUp
    }
}

#[cfg(not(target_os = "linux"))]
fn pill_kind(_cx: &App) -> WindowKind {
    WindowKind::PopUp
}
