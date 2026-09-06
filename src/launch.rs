// SPDX-License-Identifier: GPL-3.0-only

//! Application launching and activation.

use crate::app::{Applet, Message};
use crate::apps::ApplicationEntry;
use cosmic::Action;
use cosmic::app::Task;
use cosmic::applet::token::subscription::{TokenRequest, TokenUpdate};
use cosmic::surface::action::destroy_popup;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

impl Applet {
    pub(crate) fn launch_application(&mut self, app: Arc<ApplicationEntry>) -> Task<Message> {
        self.config.add_recent(&app.id);
        self.save_config();
        self.rebuild_nav_model();

        let launch = self.request_activation_token(app);
        if let Some(popup) = self.popup.take() {
            Task::batch([
                launch,
                Task::done(Action::App(Message::Surface(destroy_popup(popup)))),
            ])
        } else {
            launch
        }
    }

    fn request_activation_token(&mut self, app: Arc<ApplicationEntry>) -> Task<Message> {
        if let Some(sender) = &self.activation_token_sender {
            self.next_launch_request = self.next_launch_request.wrapping_add(1);
            let request_id = format!("kickoff-launch-{}", self.next_launch_request);
            let app_id = app.id.strip_suffix(".desktop").unwrap_or(&app.id).to_string();
            self.pending_launches.insert(request_id.clone(), app.clone());
            if sender
                .send(TokenRequest {
                    app_id,
                    exec: request_id.clone(),
                })
                .is_ok()
            {
                return Task::none();
            }
            self.pending_launches.remove(&request_id);
            tracing::warn!("Activation-token channel closed; launching without a token");
        }
        spawn_application(app, None)
    }

    pub(crate) fn handle_activation_token(&mut self, update: TokenUpdate) -> Task<Message> {
        match update {
            TokenUpdate::Init(sender) => {
                self.activation_token_sender = Some(sender);
                Task::none()
            }
            TokenUpdate::ActivationToken { token, exec } => self
                .pending_launches
                .remove(&exec)
                .map_or_else(Task::none, |app| spawn_application(app, token)),
            TokenUpdate::Finished => {
                self.activation_token_sender = None;
                let tasks = self
                    .pending_launches
                    .drain()
                    .map(|(_, app)| spawn_application(app, None));
                Task::batch(tasks)
            }
        }
    }
}

fn spawn_application(app: Arc<ApplicationEntry>, token: Option<String>) -> Task<Message> {
    Task::future(async move {
        let app_id = app.id.strip_suffix(".desktop").unwrap_or(&app.id).to_string();
        if app.dbus_activatable {
            match activate_dbus_application(&app_id, token.as_deref()).await {
                Ok(()) => return cosmic::action::app(Message::LaunchFinished),
                Err(error) => {
                    tracing::warn!("D-Bus activation failed for {}: {error}", app.id);
                }
            }
        }

        let Some(exec) = app.exec.clone() else {
            tracing::warn!("{} has no Exec fallback", app.id);
            return cosmic::action::app(Message::LaunchFinished);
        };
        let mut environment = Vec::new();
        if let Some(token) = &token {
            environment.push(("XDG_ACTIVATION_TOKEN", token.clone()));
            environment.push(("DESKTOP_STARTUP_ID", token.clone()));
        }
        if app.working_dir.is_some() {
            if let Some(mut command) = command_in_working_dir(&app, &exec) {
                command.envs(environment);
                if cosmic::process::spawn(command).await.is_none() {
                    tracing::warn!("Failed to spawn {}", app.id);
                }
            }
        } else {
            cosmic::desktop::spawn_desktop_exec(
                exec,
                environment,
                Some(&app_id),
                app.is_terminal,
            )
            .await;
        }
        cosmic::action::app(Message::LaunchFinished)
    })
}

async fn activate_dbus_application(app_id: &str, token: Option<&str>) -> zbus::Result<()> {
    let connection = zbus::Connection::session().await?;
    let object_path = format!("/{}", app_id.replace('.', "/").replace('-', "_"));
    let proxy = zbus::Proxy::new(
        &connection,
        app_id,
        object_path.as_str(),
        "org.freedesktop.Application",
    )
    .await?;
    let mut platform_data = HashMap::new();
    if let Some(token) = token {
        platform_data.insert("activation-token", zbus::zvariant::Value::from(token));
        platform_data.insert("desktop-startup-id", zbus::zvariant::Value::from(token));
    }
    proxy.call::<_, _, ()>("Activate", &platform_data).await
}

/// libcosmic's desktop helper intentionally has no working-directory argument.
fn command_in_working_dir(app: &ApplicationEntry, exec: &str) -> Option<Command> {
    let Some(exec) = expand_exec_fields(exec, &app.name, app.icon.as_deref(), &app.path) else {
        tracing::warn!("Invalid Exec field in {}", app.id);
        return None;
    };
    let Some(mut argv) = shlex::split(&exec).filter(|argv| !argv.is_empty()) else {
        tracing::warn!("Could not parse Exec field in {}", app.id);
        return None;
    };
    if app.is_terminal {
        argv.insert(0, "-e".to_string());
        argv.insert(0, "cosmic-term".to_string());
    }
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    if let Some(directory) = &app.working_dir {
        command.current_dir(directory);
    }
    Some(command)
}

fn expand_exec_fields(
    exec: &str,
    name: &str,
    icon: Option<&str>,
    desktop_file: &Path,
) -> Option<String> {
    let mut result = String::with_capacity(exec.len());
    let mut chars = exec.chars();
    while let Some(character) = chars.next() {
        if character != '%' {
            result.push(character);
            continue;
        }
        match chars.next()? {
            '%' => result.push('%'),
            'f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm' => {}
            'i' => {
                if let Some(icon) = icon {
                    result.push_str("--icon ");
                    result.push_str(&shlex::try_quote(icon).ok()?);
                    result.push(' ');
                }
            }
            'c' => result.push_str(&shlex::try_quote(name).ok()?),
            'k' => {
                let path = desktop_file.to_string_lossy();
                result.push_str(&shlex::try_quote(path.as_ref()).ok()?);
            }
            _ => return None,
        }
    }
    Some(result.trim().to_string())
}

pub(crate) fn spawn_command(command: Command) -> Task<Message> {
    Task::future(async move {
        if cosmic::process::spawn(command).await.is_none() {
            tracing::warn!("Failed to spawn detached process");
        }
        cosmic::action::app(Message::LaunchFinished)
    })
}

#[cfg(test)]
mod tests {
    use super::expand_exec_fields;
    use std::path::Path;

    #[test]
    fn expands_desktop_entry_field_codes() {
        let expanded = expand_exec_fields(
            "program %% %c %i %k %f",
            "Menu Name",
            Some("app icon"),
            Path::new("/tmp/menu.desktop"),
        )
        .unwrap();
        assert_eq!(
            shlex::split(&expanded).unwrap(),
            [
                "program",
                "%",
                "Menu Name",
                "--icon",
                "app icon",
                "/tmp/menu.desktop",
            ]
        );
    }

    #[test]
    fn rejects_invalid_field_codes() {
        assert!(
            expand_exec_fields("program %x", "Name", None, Path::new("app.desktop")).is_none()
        );
        assert!(
            expand_exec_fields("program %", "Name", None, Path::new("app.desktop")).is_none()
        );
    }
}
