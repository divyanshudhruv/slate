use gpui::Context;
use std::sync::atomic::{AtomicU8, Ordering};
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
        if matches!(self.update, UpdateState::Available(_)) {
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
        self.update = UpdateState::Checking;
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

    pub(super) fn install_available_update(&mut self, version: String, cx: &mut Context<Self>) {
        self.pending_install = None;
        self.update = UpdateState::Downloading(version.clone(), 0);
        self.invalidate_info();
        cx.notify();
        // The download thread writes here; the banner polls it so progress ticks without a channel.
        let percent = Arc::new(AtomicU8::new(0));
        let reported = percent.clone();
        let install_version = version.clone();
        run_off_thread(
            cx,
            move || {
                let staged = crate::win::update::download_release_with_progress(
                    &install_version,
                    |value| {
                        reported.store(value, Ordering::Relaxed);
                    },
                )?;
                Ok::<_, String>((install_version, staged))
            },
            move |controller, result, cx| match result {
                Ok((staged_version, staged)) => {
                    controller.pending_install = Some((staged_version.clone(), staged));
                    controller.update = UpdateState::Installing(staged_version);
                    controller.invalidate_info();
                    cx.notify();
                }
                Err(err) => {
                    controller.update = UpdateState::Available(version);
                    controller.invalidate_info();
                    cx.notify();
                    controller.note_error(
                        format!("Update failed: {err}"),
                        Some(("Retry".into(), BannerAction::RetryUpdateCheck)),
                        cx,
                    );
                }
            },
        );
        let ticking = percent.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            let running = this
                .update(cx, |controller, cx| {
                    controller.report_download(ticking.as_ref(), cx)
                })
                .unwrap_or(false);
            if !running {
                break;
            }
        })
        .detach();
    }

    /// Copies the download counter into the state so the banner can show the same number.
    fn report_download(&mut self, percent: &AtomicU8, cx: &mut Context<Self>) -> bool {
        let (version, seen) = match &self.update {
            UpdateState::Downloading(version, seen) => (version.clone(), *seen),
            _ => return false,
        };
        let arrived = percent.load(Ordering::Relaxed);
        if arrived != seen {
            self.update = UpdateState::Downloading(version, arrived);
            self.invalidate_info();
            cx.notify();
        }
        true
    }

    /// Restart on the Ready banner: hand the staged build to the swap helper, then quit.
    pub(super) fn restart_for_update(&mut self, cx: &mut Context<Self>) {
        let Some((version, staged)) = self.pending_install.clone() else {
            return;
        };
        let canonical = config::canonical_exe_path();
        let pid = std::process::id();
        if !config::save_install(&config::InstallRecord {
            path: canonical.to_string_lossy().into_owned(),
            version,
            pid,
        }) {
            self.note_error(
                "Could not write the install record, so the update was not applied.",
                None,
                cx,
            );
            return;
        }
        if let Err(err) = crate::win::update::spawn_swap_helper(pid, &canonical, &staged) {
            self.note_error(format!("Could not restart for the update: {err}"), None, cx);
            return;
        }
        self.shutdown();
        cx.quit();
    }

    fn apply_update_result(
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
                self.update = UpdateState::Available(version);
            }
            Ok(_) => {
                self.settings.updates.latest = None;
                self.save_settings();
                self.update = UpdateState::UpToDate;
                if notify {
                    self.note_success(
                        format!("Slate v{} is the latest release.", update::CURRENT_VERSION),
                        cx,
                    );
                }
            }
            Err(err) => {
                self.update = UpdateState::Failed;
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
