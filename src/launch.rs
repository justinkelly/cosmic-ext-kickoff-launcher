// SPDX-License-Identifier: GPL-3.0-only

//! Application launching: Exec field-code expansion and process spawning.

use crate::app::{Applet, Message};
use crate::apps::ApplicationEntry;
use cosmic::app::Task;
use cosmic::surface::action::destroy_popup;
use cosmic::Action;
use std::sync::Arc;

impl Applet {
    /// Launch an application, tracking it in recents and closing the popup.
    pub fn launch_application(&mut self, app: Arc<ApplicationEntry>) -> Task<Message> {
        // Track in recents
        self.config.add_recent(&app.id);
        self.save_config();
        // Rebuild nav to show/hide Recents entry
        self.rebuild_nav_model();

        if let Some(exec) = &app.exec {
            // Expand desktop-entry Exec field codes per the spec:
            //   %% → literal %
            //   %f/%F/%u/%U → removed (no files/URLs passed by a launcher)
            //   %i → --icon <icon>
            //   %c → translated name (use Name)
            //   %k → desktop file path
            //   %d/%D/%n/%N/%v/%m → deprecated, removed
            let expanded = expand_exec_fields(exec, &app);
            if let Some(argv) = shlex::split(&expanded) {
                if !argv.is_empty() {
                    let (program, args) = if app.is_terminal {
                        // Prepend cosmic-term wrapper
                        let mut full_args = vec![String::from("cosmic-term"), String::from("--")];
                        full_args.extend(argv.clone());
                        (String::from("cosmic-term"), full_args)
                    } else {
                        (argv[0].clone(), argv)
                    };
                    let mut cmd = std::process::Command::new(&program);
                    if args.len() > 1 {
                        cmd.args(&args[1..]);
                    }
                    // Detach: don't block the applet, ignore exit status.
                    // Errors (e.g. binary not found) are logged and swallowed
                    // because there is no meaningful recovery in a launcher.
                    if let Err(e) = cmd.spawn() {
                        tracing::warn!("Failed to launch '{}': {}", program, e);
                    }
                }
            }
        }
        if let Some(p) = self.popup.take() {
            return Task::done(Action::App(Message::Surface(destroy_popup(p))));
        }
        Task::none()
    }
}

/// Expand desktop-entry Exec field codes per the Freedesktop spec.
///
/// Field codes handled:
///   `%%`  → literal `%`
///   `%f`  → removed (single file — launcher passes none)
///   `%F`  → removed (file list — launcher passes none)
///   `%u`  → removed (single URL — launcher passes none)
///   `%U`  → removed (URL list — launcher passes none)
///   `%i`  → `--icon <icon_name>`
///   `%c`  → translated name (uses the Name field)
///   `%k`  → desktop file path
///   `%d`, `%D`, `%n`, `%N`, `%v`, `%m` → removed (deprecated)
fn expand_exec_fields(exec: &str, app: &ApplicationEntry) -> String {
    let mut result = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }
        // Peek at the next character after %
        match chars.next() {
            None => {
                // Trailing % — keep as-is (spec says undefined behaviour)
                result.push('%');
            }
            Some('%') => result.push('%'),
            Some('f') | Some('F') | Some('u') | Some('U') => {
                // File/URL placeholders — launcher passes nothing, remove
            }
            Some('i') => {
                if let Some(ref icon) = app.icon {
                    result.push_str("--icon ");
                    result.push_str(&shell_escape(icon));
                    result.push(' ');
                }
            }
            Some('c') => {
                // Translated name — use the Name field as a reasonable default
                result.push_str(&shell_escape(&app.name));
            }
            Some('k') => {
                // Desktop file path
                if let Some(path_str) = app.path.to_str() {
                    result.push_str(&shell_escape(path_str));
                }
            }
            Some('d') | Some('D') | Some('n') | Some('N') | Some('v') | Some('m') => {
                // Deprecated codes — remove
            }
            Some(other) => {
                // Unknown code — keep as-is per spec
                result.push('%');
                result.push(other);
            }
        }
    }

    result.trim().to_string()
}

/// Minimal shell-escaping for a single argument value (used by %c, %k, and %i).
/// Puts the value in single quotes, escaping any embedded single quotes.
fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    // Only escape if needed
    if value
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/')
    {
        return value.to_string();
    }
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}
