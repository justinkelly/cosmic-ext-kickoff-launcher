// SPDX-License-Identifier: GPL-3.0-only

//! Session and power actions.

use crate::app::Message;
use crate::fl;
use cosmic::app::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PowerAction {
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
        fn suspend(&self, interactive: bool) -> zbus::Result<()>;
        fn reboot(&self, interactive: bool) -> zbus::Result<()>;
        fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    }

    #[zbus::proxy(
        interface = "org.freedesktop.login1.Session",
        default_service = "org.freedesktop.login1",
        default_path = "/org/freedesktop/login1/session/auto"
    )]
    pub trait Session {
        fn lock(&self) -> zbus::Result<()>;
        fn terminate(&self) -> zbus::Result<()>;
    }
}

async fn manager() -> zbus::Result<login1::ManagerProxy<'static>> {
    let connection = zbus::connection::Builder::system()?.build().await?;
    login1::ManagerProxy::new(&connection).await
}

async fn session() -> zbus::Result<login1::SessionProxy<'static>> {
    let connection = zbus::connection::Builder::system()?.build().await?;
    login1::SessionProxy::new(&connection).await
}

async fn run_osd(action: &str) -> bool {
    match tokio::process::Command::new("cosmic-osd").arg(action).status().await {
        Ok(status) if status.success() => true,
        Ok(status) => {
            tracing::warn!("cosmic-osd {action} exited with {status}");
            false
        }
        Err(err) => {
            tracing::warn!("Failed to run cosmic-osd {action}: {err}");
            false
        }
    }
}

async fn run_action(action: PowerAction) -> zbus::Result<()> {
    match action {
        PowerAction::Lock => session().await?.lock().await,
        PowerAction::Logout => {
            if run_osd("log-out").await {
                Ok(())
            } else {
                session().await?.terminate().await
            }
        }
        PowerAction::Suspend => manager().await?.suspend(true).await,
        PowerAction::Restart => {
            if run_osd("restart").await {
                Ok(())
            } else {
                manager().await?.reboot(true).await
            }
        }
        PowerAction::Shutdown => {
            if run_osd("shutdown").await {
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

pub(crate) fn execute(action: PowerAction) -> Task<Message> {
    Task::future(async move {
        run(action).await;
        cosmic::action::app(Message::PowerActionDone)
    })
}
