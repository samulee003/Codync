//! Which driver runs the bots' computer tools: the default (cua-driver, one bot at a time) or agent-desktop (several
//! bots at once, in the background). The user picks it in Settings → Computer access (`setComputerDriver`, from this
//! computer only); `CODYNC_COMPUTER_DRIVER=agent-desktop` in the host's environment still forces agent-desktop.

use super::{Screen, agent_desktop};
use crate::LockExt;
use crate::store::Store;
use anyhow::Result;
use serde::{Deserialize, Serialize};

const KV_DRIVER: &str = "computer_driver";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ComputerDriver {
    #[default]
    Default,
    AgentDesktop,
}

impl ComputerDriver {
    /// A wire value (`default` / `agentDesktop`); anything else is refused.
    pub fn parse(value: &str) -> Result<Self> {
        serde_json::from_value(serde_json::json!(value))
            .map_err(|_| anyhow::anyhow!("unknown computer driver: {value}"))
    }

    fn stored(store: &Store) -> Self {
        store.kv_get(KV_DRIVER).and_then(|v| Self::parse(&v).ok()).unwrap_or_default()
    }

    fn wire(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AgentDesktop => "agentDesktop",
        }
    }
}

impl Screen {
    /// The driver the user chose (the environment may still force agent-desktop: see `agent_desktop::forced`).
    pub fn driver(&self) -> ComputerDriver {
        *self.driver_choice.locked()
    }

    /// Takes the saved choice, at start.
    pub fn restore_driver(&self, store: &Store) {
        *self.driver_choice.locked() = ComputerDriver::stored(store);
    }

    /// Saves the user's choice and tells the clients. It applies to the next computer call.
    pub fn set_driver(&self, store: &Store, driver: ComputerDriver) -> Result<()> {
        store.kv_set(KV_DRIVER, driver.wire())?;
        *self.driver_choice.locked() = driver;
        self.emit();
        Ok(())
    }

    /// The driver part of `state()`: the choice, whether the environment forces agent-desktop, and whether its CLI is
    /// installed (without it the default driver runs).
    pub(super) fn driver_state(&self) -> serde_json::Value {
        serde_json::json!({
            "driver": self.driver(),
            "driverForced": agent_desktop::forced(),
            "agentDesktopInstalled": agent_desktop::executable().is_some(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_is_saved_and_unknown_values_are_refused() {
        let store = Store::open(std::path::Path::new(":memory:")).unwrap();
        let screen = Screen::new(Some(true), tokio::sync::broadcast::channel(4).0);
        screen.restore_driver(&store);
        assert_eq!(screen.driver(), ComputerDriver::Default, "nothing saved: the default driver");

        screen.set_driver(&store, ComputerDriver::AgentDesktop).unwrap();
        let fresh = Screen::new(Some(true), tokio::sync::broadcast::channel(4).0);
        fresh.restore_driver(&store);
        assert_eq!(fresh.driver(), ComputerDriver::AgentDesktop, "kept across restarts");
        assert_eq!(fresh.state()["driver"], "agentDesktop");

        assert!(ComputerDriver::parse("cua").is_err());
        assert_eq!(ComputerDriver::parse("default").unwrap(), ComputerDriver::Default);
    }
}
