use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{
    config::AppConfig,
    reconciler::{self, ReconcileOptions},
    reporting::print_plain,
    state::State,
};

pub fn run(
    config: &AppConfig,
    config_existed: bool,
    cli_roots: &[PathBuf],
    state: &State,
    dry_run: bool,
) -> Result<()> {
    let scope = config.scan_scope(config_existed, cli_roots)?;
    let initial = reconciler::apply(
        config,
        config_existed,
        cli_roots,
        state,
        ReconcileOptions { dry_run },
    )?;
    print_plain(&initial, dry_run);

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(
        move |result| {
            let _ = tx.send(result);
        },
        NotifyConfig::default(),
    )
    .context("failed to create filesystem watcher")?;

    for root in &scope.roots {
        watcher
            .watch(root, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", root.display()))?;
    }

    let event_poll_interval =
        Duration::from_secs(config.watch.reconcile_interval_minutes.max(1) * 60);
    let full_rescan_interval =
        Duration::from_secs(config.watch.full_rescan_interval_hours.max(1) * 60 * 60);
    let debounce = Duration::from_millis(500);
    let mut last_run = Instant::now();

    loop {
        let until_full_rescan = full_rescan_interval
            .checked_sub(last_run.elapsed())
            .unwrap_or(Duration::ZERO);
        let timeout = event_poll_interval.min(until_full_rescan.max(Duration::from_millis(1)));

        match rx.recv_timeout(timeout) {
            Ok(Ok(_event)) => {
                thread::sleep(debounce);
                while rx.try_recv().is_ok() {}
                run_once(config, config_existed, cli_roots, state, dry_run)?;
                last_run = Instant::now();
            }
            Ok(Err(error)) => {
                tracing::warn!("watch error: {error}");
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if last_run.elapsed() >= full_rescan_interval {
                    run_once(config, config_existed, cli_roots, state, dry_run)?;
                    last_run = Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("filesystem watcher disconnected");
            }
        }
    }
}

fn run_once(
    config: &AppConfig,
    config_existed: bool,
    cli_roots: &[PathBuf],
    state: &State,
    dry_run: bool,
) -> Result<()> {
    let report = reconciler::apply(
        config,
        config_existed,
        cli_roots,
        state,
        ReconcileOptions { dry_run },
    )?;
    print_plain(&report, dry_run);
    Ok(())
}
