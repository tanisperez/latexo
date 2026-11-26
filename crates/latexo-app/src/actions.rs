//! The keyboard shortcuts: their actions and their keys.

use gpui_kit::{App, KeyBinding, actions};

actions!(
    latexo,
    [
        /// Shows the Processes screen and puts the cursor in its search box (`Ctrl+F`).
        FocusSearch,
        /// Ends the selected process, after the confirmation (`Delete`, on the Processes screen).
        EndProcess,
        /// Shows Settings (`Ctrl+,`).
        OpenSettings,
        /// Selects the process above (`Up`, on the Processes screen).
        SelectPrevious,
        /// Selects the process below (`Down`, on the Processes screen).
        SelectNext,
    ]
);

/// The key context of the window while the Processes screen is shown.
pub const PROCESSES_CONTEXT: &str = "LatexoProcesses";

/// Binds the keys of the actions above.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-f", FocusSearch, None),
        KeyBinding::new("delete", EndProcess, Some(PROCESSES_CONTEXT)),
        KeyBinding::new("ctrl-,", OpenSettings, None),
        KeyBinding::new("up", SelectPrevious, Some(PROCESSES_CONTEXT)),
        KeyBinding::new("down", SelectNext, Some(PROCESSES_CONTEXT)),
    ]);
}
