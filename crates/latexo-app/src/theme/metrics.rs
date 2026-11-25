//! Named layout constants, from the design (`docs/design-system.md`). Views read sizes only from
//! here, never as a literal.

// The sizes of the standard components and the radius scale, shared with the other apps.
#[allow(unused_imports)]
pub use telaio_ui::metrics::*;

/// Height of the title bar.
pub const TITLE_BAR_HEIGHT: f32 = 40.0;
/// Side of the title bar's settings button.
pub const TITLE_BUTTON_SIZE: f32 = 28.0;
/// Width of the side bar.
pub const SIDEBAR_WIDTH: f32 = 232.0;
/// Padding of the side bar.
pub const SIDEBAR_PADDING: f32 = 12.0;
/// Height of a side bar entry.
pub const NAV_ROW_HEIGHT: f32 = 30.0;
/// Side of the app icon in the side bar.
pub const SIDEBAR_LOGO_SIZE: f32 = 24.0;
/// Padding of the content area: vertical, horizontal.
pub const CONTENT_PADDING_Y: f32 = 24.0;
pub const CONTENT_PADDING_X: f32 = 28.0;
/// Space between the cards of a screen.
pub const GAP: f32 = 16.0;
/// Padding inside a card.
pub const CARD_PADDING: f32 = 16.0;
/// Side of the icon tile at the top left of a card.
pub const CARD_ICON_SIZE: f32 = 28.0;
/// Size of the big value of a card.
pub const VALUE_TEXT_SIZE: f32 = 28.0;
/// Size of a smaller big value (GPU, fans).
pub const VALUE_TEXT_SIZE_SM: f32 = 24.0;
/// Size of the screen title.
pub const TITLE_TEXT_SIZE: f32 = 20.0;
/// Padding of the chart well: vertical, horizontal.
pub const CHART_WELL_PADDING_Y: f32 = 10.0;
pub const CHART_WELL_PADDING_X: f32 = 12.0;
/// Height of the chart of an overview card (six dot rows of 8 px).
pub const OVERVIEW_CHART_HEIGHT: f32 = 48.0;
/// Height of the chart of a smaller overview card.
pub const OVERVIEW_CHART_HEIGHT_SM: f32 = 32.0;
/// Height of the big chart of a detail screen.
pub const DETAIL_CHART_HEIGHT: f32 = 112.0;
/// Width of the y axis labels of a detail chart.
pub const AXIS_WIDTH: f32 = 52.0;
/// Height of the chart of a GPU or fan card.
pub const SMALL_CHART_HEIGHT: f32 = 48.0;
/// Height of a per core chart.
pub const CORE_CHART_HEIGHT: f32 = 34.0;
/// Height of a bar of a list (usage of a drive, a sensor).
pub const BAR_HEIGHT: f32 = 6.0;
/// Height of a key and value row.
pub const STAT_ROW_HEIGHT: f32 = 30.0;
/// Height of a row of the busiest processes.
pub const BUSY_ROW_HEIGHT: f32 = 28.0;
/// Height of a settings row.
pub const SETTINGS_ROW_HEIGHT: f32 = 42.0;
/// Height of an alert rule row.
pub const RULE_ROW_HEIGHT: f32 = 54.0;
/// Height of a row of the cores table.
pub const CORE_ROW_HEIGHT: f32 = 28.0;
/// Height of a heat map row.
pub const HEAT_ROW_HEIGHT: f32 = 18.0;
/// Height of a processor map tile.
pub const CORE_TILE_HEIGHT: f32 = 88.0;
/// Width of the search box of the process table.
pub const PROCESS_SEARCH_WIDTH: f32 = 210.0;
/// Side of a legend swatch.
pub const SWATCH_SIZE: f32 = 8.0;
/// Size of the small text: labels, descriptions, legends.
pub const TEXT_SM: f32 = 12.0;
/// Size of the body text of rows.
pub const TEXT_ROW: f32 = 12.5;
/// Size of the axis labels.
pub const TEXT_AXIS: f32 = 10.5;
/// Size of the side bar group labels.
pub const TEXT_GROUP: f32 = 11.0;
/// Height of a dot row of the dotted charts (6 px dot and 2 px gap).
pub const DOT_PITCH: f32 = 8.0;
/// Size of a dot of the dotted charts.
pub const DOT_SIZE: f32 = 6.0;
/// The smallest window.
pub const MIN_WINDOW: (f32, f32) = (1024.0, 680.0);
