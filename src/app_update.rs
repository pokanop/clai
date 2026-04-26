//! `clai self update` via GitHub Releases (optional).

use self_update::update::ReleaseUpdate;

use crate::error::{AppError, Result};

pub fn self_update(
    repo_owner: &str,
    repo_name: &str,
    bin_name: &str,
    current_version: &str,
    dry_run: bool,
) -> Result<()> {
    let upd = self_update::backends::github::Update::configure()
        .repo_owner(repo_owner)
        .repo_name(repo_name)
        .bin_name(bin_name)
        .show_download_progress(true)
        .show_output(true)
        .current_version(current_version)
        .no_confirm(true)
        .build()
        .map_err(|e| AppError::Msg(format!("self_update configure: {}", e)))?;

    if dry_run {
        let rel = ReleaseUpdate::get_latest_release(&*upd)
            .map_err(|e| AppError::Msg(format!("release: {}", e)))?;
        println!(
            "latest tag: {} (current {})",
            rel.version,
            current_version
        );
        return Ok(());
    }

    let status = ReleaseUpdate::update(&*upd)
        .map_err(|e| AppError::Msg(format!("self_update: {}", e)))?;
    match status {
        self_update::Status::UpToDate(s) => println!("already up to date: {}", s),
        self_update::Status::Updated(s) => println!("updated to {}", s),
    }
    Ok(())
}
