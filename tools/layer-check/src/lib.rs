//! Checks that Halberd's crates follow the architecture rules.
//!
//! The rules live in `ARCHITECTURE.md`. In short: every crate declares a
//! layer from 0 (file formats) to 4 (the app), and may only depend on
//! Halberd crates in its own layer or below. Every crate also uses the
//! workspace lints and documents its purpose and limits in a README.
//!
//! The checks are plain functions over parsed data, so they can be tested
//! on made-up crates as well as on the real workspace.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// The highest layer number (the app).
pub const MAX_LAYER: i64 = 4;

/// Prefix shared by every Halberd crate name.
pub const CRATE_PREFIX: &str = "halberd-";

/// What the checker knows about one crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateInfo {
    /// Package name, such as `halberd-vmf`.
    pub name: String,
    /// Declared layer, or `None` if the manifest does not declare one.
    pub layer: Option<i64>,
    /// Names of the Halberd crates this crate depends on (any dependency kind).
    pub halberd_deps: Vec<String>,
    /// Whether the manifest opts into the workspace lints.
    pub uses_workspace_lints: bool,
    /// Whether the README exists and has both required sections.
    pub readme_ok: bool,
}

/// One broken rule, described in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// The crate declares no layer, or one outside 0 to 4.
    MissingOrInvalidLayer {
        /// The crate at fault.
        krate: String,
    },
    /// The crate depends on a crate in a higher layer.
    DependsUpward {
        /// The crate at fault.
        krate: String,
        /// Its layer.
        layer: i64,
        /// The higher-layer crate it depends on.
        dependency: String,
        /// That crate's layer.
        dependency_layer: i64,
    },
    /// The crate depends on a Halberd crate that does not exist in `crates/`.
    UnknownDependency {
        /// The crate at fault.
        krate: String,
        /// The missing dependency.
        dependency: String,
    },
    /// The crate does not use the workspace lints.
    NoWorkspaceLints {
        /// The crate at fault.
        krate: String,
    },
    /// The crate's README is missing or lacks a required section.
    ReadmeIncomplete {
        /// The crate at fault.
        krate: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingOrInvalidLayer { krate } => write!(
                f,
                "{krate}: add `[package.metadata.halberd] layer = N` (0 to {MAX_LAYER}) to its Cargo.toml"
            ),
            Self::DependsUpward {
                krate,
                layer,
                dependency,
                dependency_layer,
            } => write!(
                f,
                "{krate} (layer {layer}) depends on {dependency} (layer {dependency_layer}); \
                 crates may only depend on their own layer or below"
            ),
            Self::UnknownDependency { krate, dependency } => {
                write!(
                    f,
                    "{krate} depends on {dependency}, which is not a crate in crates/"
                )
            }
            Self::NoWorkspaceLints { krate } => {
                write!(
                    f,
                    "{krate}: add `[lints] workspace = true` to its Cargo.toml"
                )
            }
            Self::ReadmeIncomplete { krate } => write!(
                f,
                "{krate}: README.md must exist and contain `## Purpose` and `## Must never`"
            ),
        }
    }
}

/// Why the workspace could not be read.
#[derive(Debug)]
pub enum LoadError {
    /// A file or folder could not be read.
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A `Cargo.toml` is not valid TOML or has no package name.
    BadManifest {
        /// The manifest that failed.
        path: PathBuf,
        /// What was wrong.
        reason: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "could not read {}: {source}", path.display()),
            Self::BadManifest { path, reason } => {
                write!(f, "invalid manifest {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// Parses one crate's `Cargo.toml` text. `readme` is the README text, if any.
pub fn parse_crate(manifest: &str, readme: Option<&str>) -> Result<CrateInfo, String> {
    let doc: toml::Table = manifest.parse().map_err(|e| format!("{e}"))?;
    let package = doc
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "no [package] section".to_string())?;
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "no package name".to_string())?
        .to_string();
    let layer = package
        .get("metadata")
        .and_then(|m| m.get("halberd"))
        .and_then(|h| h.get("layer"))
        .and_then(toml::Value::as_integer);

    let mut halberd_deps = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = doc.get(section).and_then(toml::Value::as_table) {
            for dep in deps.keys().filter(|k| k.starts_with(CRATE_PREFIX)) {
                if !halberd_deps.contains(dep) {
                    halberd_deps.push(dep.clone());
                }
            }
        }
    }
    halberd_deps.sort();

    let uses_workspace_lints = doc
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);

    let readme_ok =
        readme.is_some_and(|text| text.contains("## Purpose") && text.contains("## Must never"));

    Ok(CrateInfo {
        name,
        layer,
        halberd_deps,
        uses_workspace_lints,
        readme_ok,
    })
}

/// Checks every rule across a set of crates and returns all violations found.
pub fn check(crates: &[CrateInfo]) -> Vec<Violation> {
    let mut found = Vec::new();
    let layer_of = |name: &str| crates.iter().find(|c| c.name == name).map(|c| c.layer);

    for c in crates {
        let layer = match c.layer {
            Some(l) if (0..=MAX_LAYER).contains(&l) => Some(l),
            _ => {
                found.push(Violation::MissingOrInvalidLayer {
                    krate: c.name.clone(),
                });
                None
            }
        };
        if !c.uses_workspace_lints {
            found.push(Violation::NoWorkspaceLints {
                krate: c.name.clone(),
            });
        }
        if !c.readme_ok {
            found.push(Violation::ReadmeIncomplete {
                krate: c.name.clone(),
            });
        }
        for dep in &c.halberd_deps {
            match layer_of(dep) {
                None => found.push(Violation::UnknownDependency {
                    krate: c.name.clone(),
                    dependency: dep.clone(),
                }),
                Some(Some(dep_layer)) => {
                    if let Some(own) = layer
                        && dep_layer > own
                    {
                        found.push(Violation::DependsUpward {
                            krate: c.name.clone(),
                            layer: own,
                            dependency: dep.clone(),
                            dependency_layer: dep_layer,
                        });
                    }
                }
                // The dependency has no valid layer; that is reported on its own.
                Some(None) => {}
            }
        }
    }
    found
}

/// Reads every crate under `<workspace_root>/crates`.
pub fn load_workspace(workspace_root: &Path) -> Result<Vec<CrateInfo>, LoadError> {
    let crates_dir = workspace_root.join("crates");
    let entries = fs::read_dir(&crates_dir).map_err(|source| LoadError::Io {
        path: crates_dir.clone(),
        source,
    })?;

    let mut crates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| LoadError::Io {
            path: crates_dir.clone(),
            source,
        })?;
        let dir = entry.path();
        let manifest_path = dir.join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest = fs::read_to_string(&manifest_path).map_err(|source| LoadError::Io {
            path: manifest_path.clone(),
            source,
        })?;
        let readme = fs::read_to_string(dir.join("README.md")).ok();
        let info =
            parse_crate(&manifest, readme.as_deref()).map_err(|reason| LoadError::BadManifest {
                path: manifest_path,
                reason,
            })?;
        crates.push(info);
    }
    crates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(crates)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD_README: &str = "# x\n## Purpose\nstuff\n## Must never\n- things\n";

    fn krate(name: &str, layer: Option<i64>, deps: &[&str]) -> CrateInfo {
        CrateInfo {
            name: name.to_string(),
            layer,
            halberd_deps: deps.iter().map(|d| d.to_string()).collect(),
            uses_workspace_lints: true,
            readme_ok: true,
        }
    }

    #[test]
    fn clean_set_has_no_violations() {
        let crates = [
            krate("halberd-kv", Some(0), &[]),
            krate("halberd-vmf", Some(0), &["halberd-kv"]),
            krate("halberd-io", Some(2), &["halberd-vmf"]),
        ];
        assert!(check(&crates).is_empty());
    }

    #[test]
    fn upward_dependency_is_caught() {
        let crates = [
            krate("halberd-kv", Some(0), &["halberd-ui"]),
            krate("halberd-ui", Some(3), &[]),
        ];
        let found = check(&crates);
        assert_eq!(
            found,
            vec![Violation::DependsUpward {
                krate: "halberd-kv".into(),
                layer: 0,
                dependency: "halberd-ui".into(),
                dependency_layer: 3,
            }]
        );
    }

    #[test]
    fn same_layer_dependency_is_allowed() {
        let crates = [
            krate("halberd-a", Some(1), &["halberd-b"]),
            krate("halberd-b", Some(1), &[]),
        ];
        assert!(check(&crates).is_empty());
    }

    #[test]
    fn missing_and_out_of_range_layers_are_caught() {
        let crates = [
            krate("halberd-a", None, &[]),
            krate("halberd-b", Some(7), &[]),
            krate("halberd-c", Some(-1), &[]),
        ];
        let found = check(&crates);
        assert_eq!(found.len(), 3);
        assert!(
            found
                .iter()
                .all(|v| matches!(v, Violation::MissingOrInvalidLayer { .. }))
        );
    }

    #[test]
    fn unknown_dependency_is_caught() {
        let crates = [krate("halberd-a", Some(1), &["halberd-ghost"])];
        assert_eq!(
            check(&crates),
            vec![Violation::UnknownDependency {
                krate: "halberd-a".into(),
                dependency: "halberd-ghost".into()
            }]
        );
    }

    #[test]
    fn missing_lints_and_readme_are_caught() {
        let mut c = krate("halberd-a", Some(0), &[]);
        c.uses_workspace_lints = false;
        c.readme_ok = false;
        let found = check(&[c]);
        assert!(found.contains(&Violation::NoWorkspaceLints {
            krate: "halberd-a".into()
        }));
        assert!(found.contains(&Violation::ReadmeIncomplete {
            krate: "halberd-a".into()
        }));
    }

    #[test]
    fn parses_a_full_manifest() {
        let manifest = r#"
            [package]
            name = "halberd-io"
            [package.metadata.halberd]
            layer = 2
            [dependencies]
            halberd-vmf.workspace = true
            toml = "0.9"
            [dev-dependencies]
            halberd-kv.workspace = true
            halberd-vmf.workspace = true
            [lints]
            workspace = true
        "#;
        let info = parse_crate(manifest, Some(GOOD_README)).unwrap();
        assert_eq!(info.name, "halberd-io");
        assert_eq!(info.layer, Some(2));
        assert_eq!(info.halberd_deps, vec!["halberd-kv", "halberd-vmf"]);
        assert!(info.uses_workspace_lints);
        assert!(info.readme_ok);
    }

    #[test]
    fn manifest_without_layer_or_lints_parses_as_none() {
        let info = parse_crate("[package]\nname = \"halberd-x\"\n", None).unwrap();
        assert_eq!(info.layer, None);
        assert!(!info.uses_workspace_lints);
        assert!(!info.readme_ok);
    }

    #[test]
    fn readme_needs_both_sections() {
        let only_purpose = "## Purpose\nx\n";
        let info = parse_crate("[package]\nname = \"halberd-x\"\n", Some(only_purpose)).unwrap();
        assert!(!info.readme_ok);
    }

    #[test]
    fn broken_manifests_return_errors_not_panics() {
        assert!(parse_crate("this is [not toml", None).is_err());
        assert!(parse_crate("[dependencies]\n", None).is_err());
        assert!(parse_crate("[package]\nversion = \"1\"\n", None).is_err());
    }

    #[test]
    fn violations_explain_the_fix() {
        let text = Violation::DependsUpward {
            krate: "halberd-kv".into(),
            layer: 0,
            dependency: "halberd-ui".into(),
            dependency_layer: 3,
        }
        .to_string();
        assert!(text.contains("halberd-kv (layer 0) depends on halberd-ui (layer 3)"));
    }
}
