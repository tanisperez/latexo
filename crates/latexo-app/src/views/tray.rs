//! The tray icon of the design, with its menu: Open Latexo, the floating pill and Quit. A click
//! on the icon opens the flyout (`views/flyout.rs`). On Linux it is a StatusNotifierItem over D-Bus (`ksni`, on the zbus
//! gpui already builds; KDE and most desktops show it, GNOME needs its AppIndicator extension), on
//! Windows a notification area icon (`tray-icon`). Both send what the user picks through a
//! channel the window waits on, so the tray does no work while nobody touches it.

use gpui_kit::*;
use rust_i18n::t;

use super::root::AppView;

/// The tray icon, 32 px RGBA, from `packaging/icons/png/latexo-32.png` (`assets/tray-32.rgba`).
const ICON_RGBA: &[u8] = include_bytes!("../../assets/tray-32.rgba");
const ICON_SIZE: u32 = 32;

/// What the user picked in the tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// Bring the window to the front.
    Open,
    /// Open or close the flyout (a click on the icon).
    Flyout,
    /// Turn the floating pill on or off.
    TogglePill,
    /// Quit Latexo.
    Quit,
}

/// The menu labels, in the language of the moment.
#[derive(Debug, Clone)]
struct Labels {
    open: String,
    pill: String,
    quit: String,
}

impl Labels {
    fn now() -> Self {
        Self {
            open: t!("tray.open").into_owned(),
            pill: t!("settings.floating_pill").into_owned(),
            quit: t!("tray.quit").into_owned(),
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::menu::{CheckmarkItem, StandardItem};
    use ksni::{Icon, MenuItem, Tray};

    use super::{ICON_RGBA, ICON_SIZE, Labels, TrayCommand};

    struct LatexoTray {
        sender: async_channel::Sender<TrayCommand>,
        labels: Labels,
        pill: bool,
    }

    impl LatexoTray {
        fn send(&self, command: TrayCommand) {
            let _ = self.sender.try_send(command);
        }
    }

    impl Tray for LatexoTray {
        fn id(&self) -> String {
            latexo_cli::IDENTITY.app_id.to_string()
        }

        fn title(&self) -> String {
            "Latexo".to_string()
        }

        fn icon_name(&self) -> String {
            latexo_cli::IDENTITY.app_id.to_string()
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            // ksni wants ARGB in network byte order.
            let data = ICON_RGBA
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|[r, g, b, a]| [*a, *r, *g, *b])
                .collect();
            vec![Icon {
                width: ICON_SIZE as i32,
                height: ICON_SIZE as i32,
                data,
            }]
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            self.send(TrayCommand::Flyout);
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            vec![
                StandardItem {
                    label: self.labels.open.clone(),
                    activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::Open)),
                    ..Default::default()
                }
                .into(),
                CheckmarkItem {
                    label: self.labels.pill.clone(),
                    checked: self.pill,
                    activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::TogglePill)),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: self.labels.quit.clone(),
                    activate: Box::new(|tray: &mut Self| tray.send(TrayCommand::Quit)),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    /// The running tray icon.
    pub struct TrayIcon {
        handle: Handle<LatexoTray>,
    }

    impl TrayIcon {
        pub fn new(sender: async_channel::Sender<TrayCommand>, pill: bool) -> Result<Self, String> {
            let tray = LatexoTray {
                sender,
                labels: Labels::now(),
                pill,
            };
            tray.spawn()
                .map(|handle| Self { handle })
                .map_err(|error| error.to_string())
        }

        pub fn sync(&self, pill: bool) {
            let labels = Labels::now();
            self.handle.update(|tray| {
                tray.pill = pill;
                tray.labels = labels;
            });
        }
    }

    impl Drop for TrayIcon {
        fn drop(&mut self) {
            let _ = self.handle.shutdown();
        }
    }
}

#[cfg(windows)]
mod platform {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    use super::{ICON_RGBA, ICON_SIZE, Labels, TrayCommand};

    /// The running tray icon. It lives on the main thread, whose message loop drives it.
    pub struct TrayIcon {
        _tray: tray_icon::TrayIcon,
        open: MenuItem,
        pill: CheckMenuItem,
        quit: MenuItem,
    }

    impl TrayIcon {
        pub fn new(
            sender: async_channel::Sender<TrayCommand>,
            pill_on: bool,
        ) -> Result<Self, String> {
            let labels = Labels::now();
            let open = MenuItem::new(&labels.open, true, None);
            let pill = CheckMenuItem::new(&labels.pill, true, pill_on, None);
            let quit = MenuItem::new(&labels.quit, true, None);
            let menu = Menu::new();
            menu.append_items(&[&open, &pill, &PredefinedMenuItem::separator(), &quit])
                .map_err(|error| error.to_string())?;
            let icon = Icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE)
                .map_err(|error| error.to_string())?;
            let tray = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .with_tooltip("Latexo")
                .with_icon(icon)
                .build()
                .map_err(|error| error.to_string())?;
            let (open_id, pill_id, quit_id) =
                (open.id().clone(), pill.id().clone(), quit.id().clone());
            let menu_sender = sender.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let command = if event.id == open_id {
                    TrayCommand::Open
                } else if event.id == pill_id {
                    TrayCommand::TogglePill
                } else if event.id == quit_id {
                    TrayCommand::Quit
                } else {
                    return;
                };
                let _ = menu_sender.try_send(command);
            }));
            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    let _ = sender.try_send(TrayCommand::Flyout);
                }
            }));
            Ok(Self {
                _tray: tray,
                open,
                pill,
                quit,
            })
        }

        pub fn sync(&self, pill: bool) {
            let labels = Labels::now();
            self.open.set_text(&labels.open);
            self.pill.set_text(&labels.pill);
            self.pill.set_checked(pill);
            self.quit.set_text(&labels.quit);
        }
    }
}

pub use platform::TrayIcon;

impl AppView {
    /// Shows or hides the tray icon, and starts the task that follows its menu.
    pub(crate) fn set_tray(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !on {
            if self.tray.take().is_some() {
                log::info!("tray icon removed");
            }
            return;
        }
        if self.tray.is_some() {
            return;
        }
        let (sender, receiver) = async_channel::bounded(8);
        match TrayIcon::new(sender, self.settings.floating_pill) {
            Ok(tray) => {
                log::info!("tray icon shown");
                self.tray = Some(tray);
            }
            Err(error) => {
                log::warn!("could not show the tray icon: {error}");
                return;
            }
        }
        cx.spawn_in(window, async move |view, cx| {
            while let Ok(command) = receiver.recv().await {
                let handled = view.update_in(cx, |view, window, cx| match command {
                    TrayCommand::Open => window.activate_window(),
                    TrayCommand::Flyout => view.toggle_flyout(window, cx),
                    TrayCommand::TogglePill => {
                        let on = !view.settings.floating_pill;
                        view.change(window, cx, |settings| settings.floating_pill = on);
                        view.set_pill(on, window, cx);
                    }
                    TrayCommand::Quit => {
                        view.on_close();
                        view.close_pill(cx);
                        cx.quit();
                    }
                });
                if handled.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Brings the tray menu up to date: the pill's check and the language.
    pub(crate) fn sync_tray(&self) {
        if let Some(tray) = &self.tray {
            tray.sync(self.settings.floating_pill);
        }
    }
}
