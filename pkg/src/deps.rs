//! Making a project's dependencies importable.
//!
//! `nova.toml` can name a dependency three ways: a local `path`, a `git` URL, or a
//! bare `version`. The first two are resolved here into a directory the compiler is
//! told to search with `--module-root`; the third is not, and says so.
//!
//! # Why this shells out to `git`
//!
//! Because the alternative is an HTTP client, and this crate used to have one: it
//! served a mocked registry, reached a host that does not exist, and dragged in the
//! only non-permissive licence in the tree. `git` is already installed on any machine
//! that has source to fetch, it honours the user's own configuration and credentials,
//! and it means the package manager still contains no network code.
//!
//! # What this does not do
//!
//! No version solving across a dependency graph — a dependency's own dependencies are
//! not fetched. No lockfile yet. No registry. Each of those is named where it is
//! missing rather than implied.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::commands::CommandError;
use crate::manifest::{DependencySpec, Manifest};

/// Where fetched git dependencies live, relative to the project.
///
/// `.nova/` rather than `nova_modules/`: that name is already used by the older
/// registry commands, which write dependency *metadata* there. Mixing fetched source
/// into it would make `git clone` fail on a directory that is not empty.
const DEPENDENCY_DIR: &str = ".nova/deps";

/// One dependency, resolved to a directory on disk.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub name: String,
    /// The directory to hand to the compiler as a module root.
    pub root: PathBuf,
    /// What it was resolved from, for a message a human can act on.
    pub source: String,
}

/// Resolve every dependency to a directory, fetching the git ones.
///
/// Returns them in name order so two runs on the same manifest produce the same
/// `--module-root` arguments.
pub fn resolve(manifest: &Manifest, project: &Path) -> Result<Vec<Resolved>, CommandError> {
    let mut names: Vec<&String> = manifest.dependencies.keys().collect();
    names.sort();

    let mut resolved = Vec::new();
    for name in names {
        let spec = &manifest.dependencies[name];
        resolved.push(resolve_one(name, spec, project)?);
    }
    Ok(resolved)
}

fn resolve_one(
    name: &str,
    spec: &DependencySpec,
    project: &Path,
) -> Result<Resolved, CommandError> {
    let details = match spec {
        DependencySpec::Detailed(details) => details,
        DependencySpec::Simple(version) => {
            return Err(registry_error(name, version));
        }
    };

    if let Some(path) = &details.path {
        let root = normalise(&project.join(path));
        if !root.is_dir() {
            return Err(CommandError::BuildFailed(format!(
                "dependency `{name}` points at {}, which is not a directory",
                root.display()
            )));
        }
        return Ok(Resolved {
            name: name.to_string(),
            root,
            source: format!("path {}", root.display()),
        });
    }

    if let Some(url) = &details.git {
        let root = project.join(DEPENDENCY_DIR).join(name);
        fetch(name, url, details, &root)?;
        return Ok(Resolved {
            name: name.to_string(),
            root,
            source: format!("git {url}"),
        });
    }

    let version = details.version.clone().unwrap_or_else(|| "*".to_string());
    Err(registry_error(name, &version))
}

/// The one message for a dependency that needs a registry.
fn registry_error(name: &str, version: &str) -> CommandError {
    CommandError::BuildFailed(format!(
        "cannot resolve `{name} = \"{version}\"`: the NOVA registry is not implemented, \
         so a version-only dependency cannot be fetched.\n\
         Use a path or a git dependency instead:\n    \
         {name} = {{ path = \"../{name}\" }}\n    \
         {name} = {{ git = \"https://github.com/you/{name}\" }}"
    ))
}

/// Clone or update a git dependency into `root`.
fn fetch(
    name: &str,
    url: &str,
    details: &crate::manifest::DependencyDetails,
    root: &Path,
) -> Result<(), CommandError> {
    let wanted = details
        .rev
        .clone()
        .or_else(|| details.tag.clone())
        .or_else(|| details.branch.clone());

    if !root.join(".git").exists() {
        if let Some(parent) = root.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut clone = Command::new("git");
        clone.arg("clone").arg("--depth").arg("1");
        // `--branch` accepts a branch or a tag, not a commit.
        if let Some(reference) = details.tag.as_ref().or(details.branch.as_ref()) {
            clone.arg("--branch").arg(reference);
        }
        clone.arg(url).arg(root);
        run(name, &mut clone)?;
    }

    // A revision is not something `clone --depth 1` can check out, so it is fetched
    // explicitly. A no-op when the clone already landed on it.
    if let Some(reference) = wanted {
        let mut checkout = Command::new("git");
        checkout.arg("-C").arg(root).arg("checkout").arg(&reference);
        run(name, &mut checkout)?;
    }

    Ok(())
}

/// Run a git command, turning a failure into something a user can act on.
fn run(name: &str, command: &mut Command) -> Result<(), CommandError> {
    let rendered = format!("{command:?}");
    let output = command.output().map_err(|error| {
        CommandError::BuildFailed(format!(
            "could not run `git` for dependency `{name}`: {error}\n\
             git is needed to fetch a git dependency; install it, or use a path dependency."
        ))
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(CommandError::BuildFailed(format!(
        "`{rendered}` failed for dependency `{name}`:\n{}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

/// Resolve `..` and `.` without requiring the path to exist yet.
fn normalise(path: &Path) -> PathBuf {
    // `canonicalize` needs the directory to exist, which is what the caller is about
    // to check; this keeps the error message readable when it does not.
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DependencyDetails, DependencySpec};

    fn manifest_with(name: &str, spec: DependencySpec) -> Manifest {
        let mut manifest: Manifest =
            toml::from_str("[package]\nname = \"demo\"\nversion = \"0.1.0\"\n")
                .expect("a minimal manifest parses");
        manifest.dependencies.insert(name.to_string(), spec);
        manifest
    }

    #[test]
    fn a_version_only_dependency_names_the_registry_as_the_problem() {
        let manifest = manifest_with("utils", DependencySpec::Simple("1.0".to_string()));
        let error = resolve(&manifest, Path::new(".")).expect_err("must fail");
        let message = error.to_string();
        assert!(message.contains("registry is not implemented"), "{message}");
        assert!(message.contains("path = "), "must suggest path: {message}");
        assert!(message.contains("git = "), "must suggest git: {message}");
    }

    #[test]
    fn a_path_dependency_resolves_to_its_directory() {
        let root = std::env::temp_dir().join(format!("nova-deps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("project");
        let dependency = root.join("utils");
        std::fs::create_dir_all(&project).expect("project");
        std::fs::create_dir_all(&dependency).expect("dependency");

        let manifest = manifest_with(
            "utils",
            DependencySpec::Detailed(DependencyDetails {
                path: Some(PathBuf::from("../utils")),
                ..DependencyDetails::default()
            }),
        );

        let resolved = resolve(&manifest, &project).expect("a path dependency resolves");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "utils");
        assert!(resolved[0].root.ends_with("utils"), "{:?}", resolved[0].root);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_dependency_that_does_not_exist_says_which_path() {
        let manifest = manifest_with(
            "missing",
            DependencySpec::Detailed(DependencyDetails {
                path: Some(PathBuf::from("./nowhere")),
                ..DependencyDetails::default()
            }),
        );
        let error = resolve(&manifest, Path::new("/tmp")).expect_err("must fail");
        let message = error.to_string();
        assert!(message.contains("`missing`"), "{message}");
        assert!(message.contains("nowhere"), "must name the path: {message}");
    }
}
