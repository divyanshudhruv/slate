use ely_gpui_component::feedback::Toast;
use ely_gpui_component::primitives::Severity;
use gpui::Context;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::dto::{update_toast, Dialog};
use super::Controller;
use crate::config;
use crate::core::update::{self, UpdateState, CHECK_COOLDOWN_SECS};
use crate::win::process_alive;

/// Runs blocking network work on a plain thread so gpui's executor keeps ticking.
fn run_off_thread<T: Send + 'static>(
    cx: &mut Context<Controller>,
    work: impl FnOnce() -> T + Send + 'static,
    done: impl FnOnce(&mut Controller, T, &mut Context<Controller>) + 'static,
) {
    let slot = Arc::new(Mutex::new(None));
    let producer = {
        let slot = slot.clone();
        std::thread::spawn(move || {
            let result = work();
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        })
    };
    cx.spawn(async move |this, cx| {
        while !producer.is_finished() {
            cx.background_executor()
                .timer(Duration::from_millis(20))
                .await;
        }
        let result = slot.lock().ok().and_then(|mut slot| slot.take());
        if let Some(result) = result {
            let _ = this.update(cx, |controller, cx| done(controller, result, cx));
        }
    })
    .detach();
}

pub(super) fn startup_install() -> crate::core::install::Plan {
    let current_exe = std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let record = config::load_install();
    let registered = record.as_ref().map(|r| crate::core::install::Record {
        path: r.path.as_str(),
        version: r.version.as_str(),
        pid: r.pid,
    });
    let record_canonical_alive = registered.is_some_and(|r| {
        r.pid != std::process::id()
            && r.path
                .eq_ignore_ascii_case(config::canonical_exe_path().to_string_lossy().as_ref())
            && process_alive(r.pid)
    });
    let canonical_exists = config::canonical_exe_path().exists();
    let plan = crate::core::install::classify(crate::core::install::Situation {
        exe: current_exe.as_str(),
        canonical: config::canonical_exe_path().to_string_lossy().as_ref(),
        canonical_exists,
        record: registered,
        record_canonical_alive,
    });
    plan
}

impl Controller {
    pub(super) fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if matches!(self.update, UpdateState::Available(_)) {
            self.dialog = Some(Dialog::Update);
            self.invalidate_info();
            cx.notify();
            return;
        }
        let now = update::now_secs();
        if now.saturating_sub(self.settings.updates.checked_at) < CHECK_COOLDOWN_SECS {
            self.push_toast(
                update_toast(
                    "Checked a moment ago",
                    Severity::Info,
                    "Slate checked just now - try again shortly.",
                ),
                cx,
            );
            return;
        }
        self.settings.updates.checked_at = now;
        self.save_settings();
        self.update = UpdateState::Checking;
        self.invalidate_info();
        cx.notify();
        run_off_thread(
            cx,
            crate::win::update::fetch_latest,
            |controller, result, cx| controller.apply_update_result(result, cx),
        );
    }

    pub(super) fn install_update_from_dialog(&mut self, version: String, cx: &mut Context<Self>) {
        self.install_available_update(version, cx);
    }

    fn install_available_update(&mut self, version: String, cx: &mut Context<Self>) {
        self.push_toast(
            Toast::new(format!("Downloading v{version}"))
                .severity(Severity::Info)
                .body("Downloading and verifying update..."),
            cx,
        );
        let install_version = version.clone();
        run_off_thread(
            cx,
            move || {
                let downloaded = crate::win::update::download_release(&install_version)?;
                let canonical = config::canonical_exe_path();
                let pid = std::process::id();
                crate::win::update::spawn_swap_helper(pid, &canonical, &downloaded)?;
                if !config::save_install(&config::InstallRecord {
                    path: canonical.to_string_lossy().into_owned(),
                    version: install_version.clone(),
                    pid,
                }) {
                    return Err("failed to write install record".to_string());
                }
                Ok(install_version)
            },
            move |controller, result, cx| match result {
                Ok(v) => {
                    controller.push_toast(
                        Toast::new("Installing update")
                            .severity(Severity::Success)
                            .body(format!("v{v} verified. Slate will restart shortly.")),
                        cx,
                    );
                    controller.invalidate_info();
                    cx.notify();
                }
                Err(err) => {
                    controller.update = UpdateState::Available(version);
                    controller.invalidate_info();
                    cx.notify();
                    controller.push_toast(update_toast("Update failed", Severity::Danger, err), cx);
                }
            },
        );
    }

    fn apply_update_result(&mut self, result: Result<String, String>, cx: &mut Context<Self>) {
        match result {
            Ok(version) if update::is_newer(update::CURRENT_VERSION, &version) => {
                self.settings.updates.latest = Some(version.clone());
                self.save_settings();
                self.update = UpdateState::Available(version.clone());
                self.dialog = Some(Dialog::Update);
            }
            Ok(_) => {
                self.settings.updates.latest = None;
                self.save_settings();
                self.update = UpdateState::UpToDate;
                let body = format!("Slate v{} is the latest release.", update::CURRENT_VERSION);
                self.push_toast(update_toast("Up to date", Severity::Success, body), cx);
            }
            Err(err) => {
                self.update = UpdateState::Failed;
                self.push_toast(
                    update_toast("Update check failed", Severity::Danger, err),
                    cx,
                );
            }
        }
        self.invalidate_info();
        cx.notify();
    }

    pub(super) fn push_toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toaster.update(cx, |toaster, cx| {
            toaster.push(toast, cx);
        });
    }
}
