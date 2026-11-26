//! Notifications of the operating system for the alerts, when the window is not in front:
//! freedesktop notifications over D-Bus on Linux (`notify-rust`, which gpui already builds) and
//! toast notifications on Windows. Sending runs on a short lived thread, since D-Bus may take a
//! moment; a failure is only logged.

/// Shows `body` under `summary` as a notification of the operating system.
pub fn send(summary: String, body: String) {
    let spawned = std::thread::Builder::new()
        .name("latexo-notify".to_string())
        .spawn(move || {
            let mut notification = notify_rust::Notification::new();
            notification.summary(&summary).body(&body).appname("Latexo");
            #[cfg(target_os = "linux")]
            notification.icon(latexo_cli::IDENTITY.app_id);
            #[cfg(windows)]
            notification.app_id(latexo_cli::IDENTITY.app_id);
            if let Err(error) = notification.show() {
                log::warn!("could not show a system notification: {error}");
            }
        });
    if let Err(error) = spawned {
        log::warn!("could not start the notification thread: {error}");
    }
}
