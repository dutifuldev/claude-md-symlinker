use std::path::PathBuf;

use anyhow::Result;

use crate::{
    adapters::{self, Adapter},
    config::AppConfig,
    discovery,
    git::GitRepo,
    materializer::{self, TargetState},
    reporting::{RepoResult, Report, Status},
    state::{ShimRecord, State},
};

#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub dry_run: bool,
    pub remove_if_source_missing: bool,
}

pub fn clean(
    config: &AppConfig,
    config_existed: bool,
    cli_roots: &[PathBuf],
    state: &State,
    options: CleanOptions,
) -> Result<Report> {
    let scope = config.scan_scope(config_existed, cli_roots)?;
    let repos = discovery::discover(&scope)?;
    let adapters = adapters::enabled_adapters(config);
    let mut report = Report::new(repos.len());

    for repo in repos {
        for adapter in &adapters {
            let result = clean_adapter(&repo, adapter, state, options).unwrap_or_else(|error| {
                result_for(&repo, adapter, Status::Error, error.to_string())
            });
            report.push(result);
        }
    }

    Ok(report)
}

fn clean_adapter(
    repo: &GitRepo,
    adapter: &Adapter,
    state: &State,
    options: CleanOptions,
) -> Result<RepoResult> {
    let source_exists = repo.root.join(&adapter.source).exists();
    let target_state = materializer::classify(repo, adapter)?;
    let managed = matches!(
        target_state,
        TargetState::ManagedSymlink { .. }
            | TargetState::ManagedCopy { .. }
            | TargetState::ManagedHardlink
    );

    if source_exists || !managed {
        let result = result_for(repo, adapter, Status::Kept, "nothing to clean");
        if !options.dry_run {
            record(state, repo, adapter, Status::Kept, &result.message)?;
        }
        return Ok(result);
    }

    if !options.remove_if_source_missing {
        let result = result_for(
            repo,
            adapter,
            Status::NoSource,
            "managed shim is stale; pass --remove-if-source-missing to remove it",
        );
        if !options.dry_run {
            record(state, repo, adapter, Status::NoSource, &result.message)?;
        }
        return Ok(result);
    }

    let removed = materializer::remove_target(repo, adapter, options.dry_run)?;
    let result = if removed {
        result_for(repo, adapter, Status::Cleaned, "removed stale managed shim")
    } else {
        result_for(repo, adapter, Status::Kept, "nothing to clean")
    };
    if !options.dry_run {
        record(state, repo, adapter, result.status, &result.message)?;
    }
    Ok(result)
}

fn record(
    state: &State,
    repo: &GitRepo,
    adapter: &Adapter,
    status: Status,
    message: &str,
) -> Result<()> {
    state.record(ShimRecord {
        repo,
        adapter_name: &adapter.name,
        source_rel_path: &adapter.source.to_string_lossy(),
        target_rel_path: &adapter.target.to_string_lossy(),
        materialization: None,
        content_hash: materializer::source_hash(repo, adapter)?,
        status,
        message,
    })
}

fn result_for(
    repo: &GitRepo,
    adapter: &Adapter,
    status: Status,
    message: impl Into<String>,
) -> RepoResult {
    RepoResult {
        repo: repo.root.display().to_string(),
        adapter: adapter.name.clone(),
        source: adapter.source.display().to_string(),
        target: adapter.target.display().to_string(),
        status,
        message: message.into(),
    }
}
