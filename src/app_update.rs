//! `clai self update` via GitHub Releases (optional).

use self_update::update::ReleaseUpdate;

use crate::cli_output::{cli_intro, cli_kv, cli_note, cli_section};
use crate::error::{AppError, Result};

/// `target_triple` should match release asset names (e.g. contains `x86_64-unknown-linux-gnu`).
/// Defaults to the compile-time `TARGET` used to build this binary.
///
/// `bin_path_in_archive` supports `self_update` templates: `{{ bin }}`, `{{ target }}`, `{{ version }}`.
pub fn self_update(
    repo_owner: &str,
    repo_name: &str,
    bin_name: &str,
    current_version: &str,
    dry_run: bool,
    target_triple: Option<&str>,
    bin_path_in_archive: Option<&str>,
) -> Result<()> {
    let mut b = self_update::backends::github::Update::configure();
    b.repo_owner(repo_owner)
        .repo_name(repo_name)
        .bin_name(bin_name)
        .show_download_progress(true)
        .show_output(true)
        .current_version(current_version)
        .no_confirm(true);

    if let Some(t) = target_triple {
        b.target(t);
    } else {
        b.target(env!("CLAI_BUILD_TARGET"));
    }
    if let Some(p) = bin_path_in_archive {
        b.bin_path_in_archive(p);
    }

    let upd = b
        .build()
        .map_err(|e| AppError::Msg(format!("self_update configure: {}", e)))?;

    if dry_run {
        let rel = ReleaseUpdate::get_latest_release(&*upd)
            .map_err(|e| AppError::Msg(format!("release: {}", e)))?;
        cli_intro("clai self update", "check only (no download)");
        cli_section("Releases");
        cli_kv("Latest tag", &rel.version);
        cli_kv("This binary", current_version);
        let triple = target_triple.unwrap_or(env!("CLAI_BUILD_TARGET"));
        cli_note(&format!(
            "Release asset file names should include this target triple: {triple}"
        ));
        println!();
        return Ok(());
    }

    let status =
        ReleaseUpdate::update(&*upd).map_err(|e| AppError::Msg(format!("self_update: {}", e)))?;
    match status {
        self_update::Status::UpToDate(s) => {
            cli_intro("clai self update", "already current");
            cli_note(&s);
            println!();
        }
        self_update::Status::Updated(s) => {
            cli_intro("clai self update", "install complete");
            cli_note(&s);
            println!();
        }
    }
    Ok(())
}
