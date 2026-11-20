//! Which git repository a review is in: the project's, or a submodule's (ATM-583).

use super::{Error, Project, Result};

/// The project's git, for a review's rounds.
///
/// # Errors
/// The project isn't in a git repository: reviews are of changes in git.
pub fn vcs(project: &Project) -> Result<atm_vcs::GitVcs> {
    project
        .vcs()
        .filter(atm_vcs::GitVcs::in_repository)
        .ok_or_else(|| {
            Error::failed("reviews are of changes in git: this project isn't in a git repository")
        })
}
/// The repository a round made in `cwd` is of (ATM-583): in a submodule, the submodule's (its revisions and
/// uncommitted changes are its own), its files named as the project has them; else the project's (`vcs`).
///
/// # Errors
/// `cwd` is in a submodule nested in another one: rounds are of the project's repository or a submodule of it.
pub fn repository_here(vcs: &atm_vcs::GitVcs, cwd: &std::path::Path) -> Result<atm_vcs::GitVcs> {
    match vcs.submodule_containing(cwd) {
        None => Ok(vcs.clone()),
        Some(folder) => vcs.submodule_at(&folder).ok_or_else(|| {
            Error::Invalid(format!(
                "{folder} is in a submodule inside another one: review from the outer submodule's folder, or the project's"
            ))
        }),
    }
}

/// The repository files reviewed whole are read from (ATM-583): a submodule's when every file is in it, else the
/// project's (`vcs`).
///
/// # Errors
/// `Invalid` when the files are in different repositories: each repository's are a review of their own.
pub fn repository_of_files(vcs: &atm_vcs::GitVcs, paths: &[String]) -> Result<atm_vcs::GitVcs> {
    let mut found: Option<Option<String>> = None;
    for path in paths {
        // The outermost folder on the way that is a submodule, if any
        let parts: Vec<&str> = path.split('/').collect();
        let folder = (1..parts.len())
            .map(|n| parts[..n].join("/"))
            .find(|f| vcs.submodule_at(f).is_some());
        match &found {
            None => found = Some(folder),
            Some(first) if *first == folder => {}
            Some(_) => {
                return Err(Error::Invalid(
                    "these files are in different repositories (the project's and a submodule's): review each repository's files in a review of their own".into(),
                ));
            }
        }
    }
    match found.flatten() {
        None => Ok(vcs.clone()),
        Some(folder) => vcs
            .submodule_at(&folder)
            .ok_or_else(|| Error::failed(format!("{folder}: not a submodule here"))),
    }
}
