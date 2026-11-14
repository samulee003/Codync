//! The palette's *Several bots at once*: switches the driver of the bots' computer tools between the default (one bot
//! at a time) and agent-desktop (several at once, in the background), as Settings → Computer access does on the
//! desktop. The host's `screen` event brings the new state.

use serde_json::json;

use super::{After, App};

impl App {
    pub(super) fn toggle_computer_driver(&mut self) {
        let Some(driver) = self.screen["driver"].as_str() else {
            self.flash("This computer's host can't switch drivers yet: update it");
            return;
        };
        if self.screen["driverForced"] == true {
            self.flash("CODYNC_COMPUTER_DRIVER on this host keeps agent-desktop on");
            return;
        }
        let next = if driver == "agentDesktop" { "default" } else { "agentDesktop" };
        self.call("setComputerDriver", json!({"driver": next}), After::Nothing);
        self.flash(match (next, self.screen["agentDesktopInstalled"] == true) {
            ("default", _) => "Bots take turns using the computer",
            (_, true) => "Several bots can use the computer at once",
            (_, false) => "agent-desktop isn't installed, so bots still take turns: npm install -g agent-desktop",
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(screen: serde_json::Value) -> App {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(crate::tui::net::Client::new("http://127.0.0.1:1", None), tx, String::new());
        app.screen = screen;
        app
    }

    fn hint(app: &App) -> String {
        app.hint.as_ref().map(|(text, _)| text.clone()).unwrap_or_default()
    }

    #[tokio::test]
    async fn the_switch_says_what_it_does() {
        let mut old = app(json!({"enabled": true}));
        old.toggle_computer_driver();
        assert!(hint(&old).contains("update it"), "a host before the choice");

        let mut forced = app(json!({"driver": "default", "driverForced": true}));
        forced.toggle_computer_driver();
        assert!(hint(&forced).contains("CODYNC_COMPUTER_DRIVER"));

        let mut missing = app(json!({"driver": "default", "agentDesktopInstalled": false}));
        missing.toggle_computer_driver();
        assert!(hint(&missing).contains("npm install -g agent-desktop"));

        let mut on = app(json!({"driver": "agentDesktop", "agentDesktopInstalled": true}));
        on.toggle_computer_driver();
        assert_eq!(hint(&on), "Bots take turns using the computer");
    }
}
