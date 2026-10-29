use gpui::Context;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::dto::BannerAction;
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
    pub(super) fn check_for_updates(&mut self, force: bool, cx: &mut Context<Self>) {
        if matches!(self.updater.state, UpdateState::Available(_)) {
            // The offer is already on screen as a banner; a second check would only repeat it.
            return;
        }
        let now = update::now_secs();
        if !force && now.saturating_sub(self.settings.updates.checked_at) < CHECK_COOLDOWN_SECS {
            // A background check that is already fresh stays silent; only an explicit click reports it.
            return;
        }
        self.settings.updates.checked_at = now;
        self.save_settings();
        self.updater.state = UpdateState::Checking;
        self.invalidate_info();
        cx.notify();
        let channel = self.settings.updates.channel;
        run_off_thread(
            cx,
            move || crate::win::update::fetch_latest(channel),
            move |controller, result, cx| controller.apply_update_result(result, force, cx),
        );
    }

    pub(super) fn toggle_update_channel(&mut self, cx: &mut Context<Self>) {
        self.settings.updates.channel = match self.settings.updates.channel {
            update::Channel::Stable => update::Channel::Beta,
            update::Channel::Beta => update::Channel::Stable,
        };
        self.settings.updates.checked_at = 0;
        self.save_settings();
        self.invalidate_info();
        cx.notify();
    }

    pub(super) fn apply_update_result(
        &mut self,
        result: Result<String, String>,
        notify: bool,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(version) if update::is_newer(update::CURRENT_VERSION, &version) => {
                self.settings.updates.latest = Some(version.clone());
                self.save_settings();
                // The banner follows the state, so nothing else has to announce it.
                // A version the user waved off stays hidden; a newer one shows again.
                self.updater.state = UpdateState::Available(version);
            }
            Ok(_) => {
                self.settings.updates.latest = None;
                self.save_settings();
                self.updater.state = UpdateState::UpToDate;
                if notify {
                    self.note_success(
                        format!("Slate v{} is the latest release.", update::CURRENT_VERSION),
                        cx,
                    );
                }
            }
            Err(err) => {
                self.updater.state = UpdateState::Failed;
                if notify {
                    self.note_error(
                        format!("Update check failed: {err}"),
                        Some(("Retry".into(), BannerAction::RetryUpdateCheck)),
                        cx,
                    );
                } else {
                    tracing::warn!("background update check failed: {err}");
                }
            }
        }
        self.invalidate_info();
        cx.notify();
    }
}
