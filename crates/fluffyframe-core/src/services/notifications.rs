//! Desktop notifications (original: `services/notificationChannels.ts`,
//! `notificationLog.ts`). Sent off the UI thread; failures are logged, not shown.

use crate::config::app_meta::APP_NAME;

pub fn send(title: &str, body: &str) {
    let (title, body) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        if let Err(e) = notify_rust::Notification::new()
            .appname(APP_NAME)
            .summary(&title)
            .body(&body)
            .show()
        {
            log::warn!("notification failed: {e}");
        }
    });
}
