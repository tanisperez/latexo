//! The Processes screen: a virtual table (only the visible rows are built) sorted by any column,
//! filtered by owner and searched by name or PID, and ending the selected process after a
//! confirmation.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use latexo_core::Sample;
use latexo_core::process::{ProcessColumn, ProcessFilter, ProcessInfo, tree};
use rust_i18n::t;
use telaio_ui::components::{
    ConfirmDialog, DangerButton, MenuEntries, SegmentedControl, SegmentedItem, edit_menu_area,
    no_input_menu, open_confirm_dialog,
};

use crate::state::format;
use crate::theme::metrics::{
    CONTROL_HEIGHT, INPUT_PADDING_X, PROCESS_SEARCH_WIDTH, RADIUS_LG, RADIUS_MD, TEXT_ROW, TEXT_SM,
    VIRTUAL_TABLE_HEADER_HEIGHT, VIRTUAL_TABLE_ROW_HEIGHT,
};
use crate::theme::{Palette, PaletteExt};

use super::root::AppView;
use super::widgets::palette_mono;

/// The fixed widths of the columns after the name.
const PID_WIDTH: f32 = 64.0;
const USER_WIDTH: f32 = 130.0;
const CPU_WIDTH: f32 = 76.0;
const MEMORY_WIDTH: f32 = 88.0;
const THREADS_WIDTH: f32 = 56.0;
const DISK_WIDTH: f32 = 88.0;
const COLUMN_GAP: f32 = 12.0;
const ROW_PADDING: f32 = 16.0;

fn column_label(column: ProcessColumn) -> String {
    match column {
        ProcessColumn::Name => t!("processes.name"),
        ProcessColumn::Pid => t!("processes.pid"),
        ProcessColumn::User => t!("processes.user"),
        ProcessColumn::Cpu => t!("processes.cpu"),
        ProcessColumn::Memory => t!("processes.memory"),
        ProcessColumn::Threads => t!("processes.threads"),
        ProcessColumn::Disk => t!("processes.disk"),
    }
    .into_owned()
}

fn column_width(column: ProcessColumn) -> Option<f32> {
    match column {
        ProcessColumn::Name => None,
        ProcessColumn::Pid => Some(PID_WIDTH),
        ProcessColumn::User => Some(USER_WIDTH),
        ProcessColumn::Cpu => Some(CPU_WIDTH),
        ProcessColumn::Memory => Some(MEMORY_WIDTH),
        ProcessColumn::Threads => Some(THREADS_WIDTH),
        ProcessColumn::Disk => Some(DISK_WIDTH),
    }
}

fn numeric(column: ProcessColumn) -> bool {
    !matches!(column, ProcessColumn::Name | ProcessColumn::User)
}

/// A cell of `column` holding `content`.
fn cell(column: ProcessColumn, content: impl IntoElement) -> Div {
    let cell = match column_width(column) {
        Some(width) => div().w(px(width)).flex_none(),
        None => div().flex_1().min_w_0(),
    };
    let cell = cell.truncate();
    if numeric(column) {
        cell.flex().justify_end().child(content)
    } else {
        cell.child(content)
    }
}

impl AppView {
    /// The header controls of the Processes screen: search, filter and End process.
    pub(crate) fn process_controls(
        &self,
        palette: &Palette,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut filter = SegmentedControl::new("process-filter");
        for (choice, label) in [
            (ProcessFilter::All, t!("processes.filter_all")),
            (ProcessFilter::Apps, t!("processes.filter_apps")),
            (ProcessFilter::System, t!("processes.filter_system")),
        ] {
            let weak = cx.weak_entity();
            filter = filter.item(
                SegmentedItem::new(label.into_owned())
                    .selected(self.query.filter == choice)
                    .on_click(move |_, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.query.filter = choice;
                            cx.notify();
                        });
                    }),
            );
        }
        let border_hover = palette.border_hover;
        let search = h_flex()
            .id("process-search")
            .w(px(PROCESS_SEARCH_WIDTH))
            .h(px(CONTROL_HEIGHT))
            .px(px(INPUT_PADDING_X))
            .gap(px(8.0))
            .items_center()
            .bg(palette.raised)
            .border_1()
            .border_color(palette.border_strong)
            .rounded(px(RADIUS_MD))
            .hover(move |style| style.border_color(border_hover))
            .child(
                Icon::new(IconName::Search)
                    .with_size(px(14.0))
                    .text_color(palette.fg_subtle),
            )
            .child(
                edit_menu_area(&self.search).flex_1().min_w_0().child(
                    Input::new(&self.search)
                        .appearance(false)
                        .cleanable(true)
                        .context_menu(no_input_menu()),
                ),
            );
        let weak = cx.weak_entity();
        let end = DangerButton::new("end-process", t!("processes.end"))
            .disabled(self.selected_pid.is_none())
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |view, cx| view.confirm_end(false, window, cx));
            });
        h_flex()
            .gap(px(10.0))
            .items_center()
            .child(search)
            .child(filter)
            .child(end)
            .into_any_element()
    }

    /// Asks before ending the selected process (and its children with `with_tree`).
    pub(crate) fn confirm_end(
        &mut self,
        with_tree: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pid) = self.selected_pid else {
            return;
        };
        let Some(sample) = self.monitor.latest.clone() else {
            return;
        };
        let Some(process) = sample.processes.iter().find(|p| p.pid == pid) else {
            return;
        };
        let pids = if with_tree {
            tree(&sample.processes, pid)
        } else {
            vec![pid]
        };
        let title = if with_tree {
            t!("processes.end_tree_title", name = process.name.clone())
        } else {
            t!("processes.end_title", name = process.name.clone())
        };
        let mut text = t!("processes.end_text", pid = pid).into_owned();
        if with_tree && pids.len() > 1 {
            text.push_str("\n\n");
            text.push_str(&crate::state::locale::plural(
                "processes.end_tree_count",
                pids.len() - 1,
            ));
        }
        text.push_str("\n\n");
        text.push_str(&t!(
            "processes.end_usage",
            name = process.name.clone(),
            cpu = format::percent_value(f64::from(process.cpu_percent), 1),
            memory = format::bytes(process.memory)
        ));
        let weak = cx.weak_entity();
        open_confirm_dialog(
            window,
            cx,
            ConfirmDialog {
                title: title.into_owned().into(),
                text: text.into(),
                confirm_label: t!("processes.end_confirm").into_owned().into(),
                destructive: true,
                show_cancel: true,
                icon: None,
                on_confirm: Rc::new(move |window, cx| {
                    let result = latexo_sensors::end_processes(&pids);
                    let _ = weak.update(cx, |view, cx| {
                        view.selected_pid = None;
                        if let Err(error) = result {
                            let message = match error {
                                latexo_sensors::KillError::NotFound => t!("processes.end_gone"),
                                latexo_sensors::KillError::Denied => t!("processes.end_denied"),
                            };
                            window.push_notification(
                                gpui_kit::component::notification::Notification::error(
                                    message.into_owned(),
                                ),
                                cx,
                            );
                        }
                        cx.notify();
                    });
                }),
            },
        );
    }

    pub(crate) fn render_processes(
        &self,
        sample: &Sample,
        palette: &Palette,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows: Vec<ProcessInfo> = self
            .query
            .apply(&sample.processes)
            .into_iter()
            .cloned()
            .collect();
        let count = rows.len();
        let rows = Rc::new(rows);
        let header = h_flex()
            .flex_none()
            .h(px(VIRTUAL_TABLE_HEADER_HEIGHT))
            .px(px(ROW_PADDING))
            .gap(px(COLUMN_GAP))
            .items_center()
            .bg(palette.surface)
            .border_b_1()
            .border_color(palette.border)
            .children(ProcessColumn::ALL.map(|column| {
                let sorted = self.query.column == column;
                let arrow = if self.query.ascending {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                };
                let (hover, pressed) = (palette.hover, palette.pressed);
                let label = h_flex()
                    .id(SharedString::from(format!("sort-{column:?}")))
                    .gap(px(4.0))
                    .items_center()
                    .px(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .active(move |style| style.bg(pressed))
                    .text_size(px(TEXT_SM))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if sorted { palette.fg } else { palette.fg_muted })
                    .child(column_label(column))
                    .when(sorted, |label| {
                        label.child(Icon::new(arrow).with_size(px(12.0)))
                    })
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.query.clicked(column);
                        cx.notify();
                    }));
                cell(column, label)
            }));
        let list_rows = rows.clone();
        let list = uniform_list(
            "process-rows",
            count,
            cx.processor(move |view, range: std::ops::Range<usize>, _window, cx| {
                let palette = cx.palette();
                range
                    .filter_map(|index| {
                        list_rows
                            .get(index)
                            .map(|p| view.process_row(p, &palette, cx))
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.process_scroll)
        .flex_1()
        .min_h_0();
        let sort = format!(
            "{} {}",
            column_label(self.query.column),
            if self.query.ascending { "↑" } else { "↓" }
        );
        let footer = h_flex()
            .flex_none()
            .h(px(30.0))
            .px(px(ROW_PADDING))
            .justify_between()
            .items_center()
            .border_t_1()
            .border_color(palette.border)
            .text_size(px(TEXT_SM))
            .text_color(palette.fg_muted)
            .child(
                t!(
                    "processes.footer_count",
                    shown = format::integer(count),
                    total = format::integer(sample.processes.len())
                )
                .into_owned(),
            )
            .child(t!("processes.footer_sort", column = sort).into_owned());
        v_flex()
            .flex_1()
            .min_h_0()
            .bg(palette.raised)
            .border_1()
            .border_color(palette.border)
            .rounded(px(RADIUS_LG))
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .when(count == 0, |area| {
                        area.child(super::widgets::empty_line(
                            t!("processes.none_match"),
                            palette,
                        ))
                    })
                    .child(list)
                    .child(gpui_kit::component::scroll::Scrollbar::vertical(
                        &self.process_scroll,
                    )),
            )
            .child(footer)
            .into_any_element()
    }

    fn process_row(
        &self,
        process: &ProcessInfo,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pid = process.pid;
        let selected = self.selected_pid == Some(pid);
        let (hover, pressed) = (palette.hover, palette.pressed);
        let mono = |text: String, muted: bool| {
            div()
                .font_family(palette_mono())
                .text_size(px(TEXT_SM))
                .when(muted, |div| div.text_color(palette.fg_muted))
                .child(text)
        };
        h_flex()
            .id(SharedString::from(format!("process-{pid}")))
            .w_full()
            .h(px(VIRTUAL_TABLE_ROW_HEIGHT))
            .px(px(ROW_PADDING))
            .gap(px(COLUMN_GAP))
            .items_center()
            .text_size(px(TEXT_ROW))
            .cursor_pointer()
            .when(selected, |row| row.bg(palette.accent_subtle))
            .when(!selected, |row| {
                row.hover(move |style| style.bg(hover))
                    .active(move |style| style.bg(pressed))
            })
            .child(cell(ProcessColumn::Name, process.name.clone()))
            .child(cell(ProcessColumn::Pid, mono(pid.to_string(), true)))
            .child(cell(
                ProcessColumn::User,
                div()
                    .text_size(px(TEXT_SM))
                    .text_color(palette.fg_muted)
                    .truncate()
                    .child(process.user.clone().unwrap_or_default()),
            ))
            .child(cell(
                ProcessColumn::Cpu,
                mono(
                    format::percent_value(f64::from(process.cpu_percent), 1),
                    false,
                ),
            ))
            .child(cell(
                ProcessColumn::Memory,
                mono(format::bytes(process.memory), true),
            ))
            .child(cell(
                ProcessColumn::Threads,
                mono(
                    process
                        .threads
                        .map(|threads| threads.to_string())
                        .unwrap_or_default(),
                    true,
                ),
            ))
            .child(cell(
                ProcessColumn::Disk,
                mono(format::rate(process.disk_per_sec()), true),
            ))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.selected_pid = Some(pid);
                cx.notify();
            }))
            .context_menu({
                let weak = cx.weak_entity();
                move |menu, window, cx| {
                    let _ = weak.update(cx, |view, cx| {
                        view.selected_pid = Some(pid);
                        cx.notify();
                    });
                    let end = weak.clone();
                    let end_tree = weak.clone();
                    MenuEntries::new()
                        .handler(t!("processes.end"), move |window, cx| {
                            let _ = end.update(cx, |view, cx| view.confirm_end(false, window, cx));
                        })
                        .handler(t!("processes.end_tree"), move |window, cx| {
                            let _ =
                                end_tree.update(cx, |view, cx| view.confirm_end(true, window, cx));
                        })
                        .render(menu, window, cx)
                }
            })
            .into_any_element()
    }
}
