//! The About screen: telaio's About header (logo, version, platform, telaio version, links), the
//! machine, the files Latexo writes and what it connects to.

use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;

use crate::state::format;
use crate::state::settings::{config_dir, settings_path};
use crate::theme::metrics::{GAP, TEXT_ROW};
use crate::theme::{Palette, PaletteExt};

use super::root::AppView;
use super::widgets::{card, card_title, stat_rows};

impl AppView {
    pub(crate) fn render_about(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let mono: SharedString = SharedString::new_static("Geist Mono");
        let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
        let header = telaio_app::about::header(
            &latexo_cli::display_identity(),
            &platform,
            &cx.palette(),
            &mono,
        );
        let mut machine = Vec::new();
        if let Some(info) = &self.monitor.info {
            if let Some(host) = &info.host_name {
                machine.push((t!("about.host").into_owned(), host.clone()));
            }
            machine.push((t!("about.os").into_owned(), info.os_name.clone()));
            machine.push((t!("about.kernel").into_owned(), info.os_version.clone()));
            machine.push((t!("about.cpu").into_owned(), info.cpu_brand.clone()));
            machine.push((
                t!("about.cores").into_owned(),
                match info.physical_cores {
                    Some(physical) => t!(
                        "about.cores_value",
                        cores = physical,
                        threads = info.logical_cores
                    )
                    .into_owned(),
                    None => info.logical_cores.to_string(),
                },
            ));
            machine.push((
                t!("about.memory").into_owned(),
                format::bytes(info.memory_total),
            ));
        }
        let path = |path: Option<std::path::PathBuf>| {
            path.map(|path| path.display().to_string())
                .unwrap_or_else(|| t!("common.dash").into_owned())
        };
        let files = vec![
            (
                t!("about.settings_file").into_owned(),
                path(settings_path()),
            ),
            (
                t!("about.energy_file").into_owned(),
                path(config_dir().map(|dir| dir.join("energy.toml"))),
            ),
            (
                t!("about.log_file").into_owned(),
                path(telaio_app::logging::log_path(&latexo_cli::IDENTITY)),
            ),
        ];
        v_flex()
            .gap(px(GAP))
            .child(card(palette).child(header))
            .child(
                card(palette)
                    .child(card_title(t!("about.machine")))
                    .child(stat_rows(machine, palette)),
            )
            .child(
                card(palette)
                    .child(card_title(t!("about.files")))
                    .child(stat_rows(files, palette)),
            )
            .child(
                card(palette)
                    .child(card_title(t!("about.privacy_title")))
                    .child(
                        div()
                            .text_size(px(TEXT_ROW))
                            .text_color(palette.fg_muted)
                            .child(t!("about.privacy").into_owned()),
                    ),
            )
            .into_any_element()
    }
}
