// SPDX-License-Identifier: GPL-3.0-only

//! Session and power actions, executed via `org.freedesktop.login1` (D-Bus)
//! with `cosmic-osd` as the primary path for actions that show a confirmation
//! dialog.

use crate::app::Message;
use crate::fl;
use cosmic::app::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Lock,
    Logout,
    Suspend,
    Restart,
    Shutdown,
}

impl PowerAction {
    pub const fn icon_name(self) -> &'static str {
        match self {
            PowerAction::Lock => "system-lock-screen-symbolic",
            PowerAction::Logout => "system-log-out-symbolic",
            PowerAction::Suspend => "system-suspend-symbolic",
            PowerAction::Restart => "system-reboot-symbolic",
            PowerAction::Shutdown => "system-shutdown-symbolic",
        }
    }

    pub fn label(self) -> String {
        match self {
            PowerAction::Lock => fl!("lock"),
            PowerAction::Logout => fl!("log-out"),
            PowerAction::Suspend => fl!("suspend"),
            PowerAction::Restart => fl!("restart"),
            PowerAction::Shutdown => fl!("shut-down"),
        }
    }

    /// Bottom-bar actions matching the standard COSMIC session/power applet.
    pub const BOTTOM_BAR: [Self; 5] = [
        Self::Lock,
        Self::Logout,
        Self::Suspend,
        Self::Restart,
        Self::Shutdown,
    ];
}

/// org.freedesktop.login1.Manager
mod login1 {
    #[zbus::proxy(
        interface = "org.freedesktop.login1.Manager",
        default_service = "org.freedesktop.login1",
        default_path = "/org/freedesktop/login1"
    )]
    pub trait Manager {
        fn lock_session(&self) -> zbus::Result<()>;
        fn terminate_user(&self, uid: u32) -> zbus::Result<()>;
        fn suspend(&self, interactive: bool) -> zbus::Result<()>;
        fn reboot(&self, interactive: bool) -> zbus::Result<()>;
        fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    }
}

async fn manager() -> zbus::Result<login1::ManagerProxy<'static>> {
    let connection = zbus::connection::Builder::system()?.build().await?;
    login1::ManagerProxy::new(&connection).await
}

fn current_uid() -> Option<u32> {
    // `UID` is a shell variable and often absent in GUI-launched sessions;
    // read the real uid from the kernel instead.
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let uid_line = status.lines().find(|line| line.starts_with("Uid:"))?;
    uid_line.split_whitespace().nth(1)?.parse().ok()
}

/// Try the `cosmic-osd` confirmation dialog first; fall back to login1.
fn spawn_osd(action: &str) -> bool {
    match std::process::Command::new("cosmic-osd").arg(action).spawn() {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!("Failed to spawn cosmic-osd {action}: {err}");
            false
        }
    }
}

async fn run_action(action: PowerAction) -> zbus::Result<()> {
    match action {
        PowerAction::Lock => manager().await?.lock_session().await,
        PowerAction::Logout => {
            if spawn_osd("logout") {
                Ok(())
            } else if let Some(uid) = current_uid() {
                manager().await?.terminate_user(uid).await
            } else {
                tracing::warn!("Cannot determine user id for logout");
                Ok(())
            }
        }
        PowerAction::Suspend => manager().await?.suspend(true).await,
        PowerAction::Restart => {
            if spawn_osd("restart") {
                Ok(())
            } else {
                manager().await?.reboot(true).await
            }
        }
        PowerAction::Shutdown => {
            if spawn_osd("shutdown") {
                Ok(())
            } else {
                manager().await?.power_off(true).await
            }
        }
    }
}

async fn run(action: PowerAction) {
    if let Err(err) = run_action(action).await {
        tracing::warn!("Power action {action:?} failed: {err}");
    }
}

/// Execute a power action in the background.
pub fn execute(action: PowerAction) -> Task<Message> {
    Task::future(async move {
        run(action).await;
        cosmic::action::app(Message::PowerActionDone)
    })
}

