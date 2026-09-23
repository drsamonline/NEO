//! Package manifest handling
//! 
//! Parses and validates Nova.toml manifest files, handles dependency
//! specifications, and manages project metadata.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Failed to read manifest file: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Failed to parse manifest: {0}")]
    ParseError(#[from] toml::de::Error),
    
    #[error("Failed to serialize manifest: {0}")]
    SerializeError(#[from] toml::ser::Error),
    
    #[error("Invalid version requirement: {0}")]
    InvalidVersion(String),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Package manifest structure (Nova.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Package metadata
    pub package: PackageInfo,
    
    /// Build configuration
    #[serde(default)]
    pub build: BuildConfig,
    
    /// Runtime dependencies
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,
    
    /// Development dependencies
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: HashMap<String, DependencySpec>,
    
    /// Optional platform-specific dependencies
    #[serde(default)]
    pub target: HashMap<String, TargetDependencies>,
}

/// Package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Package name
    pub name: String,
    
    /// Package version (semver)
    pub version: String,
    
    /// Package description
    #[serde(default)]
    pub description: String,
    
    /// Authors
    #[serde(default)]
    pub authors: Vec<String>,
    
    /// License identifier (e.g., "MIT", "Apache-2.0")
    #[serde(default)]
    pub license: Option<String>,
    
    /// Homepage URL
    #[serde(default)]
    pub homepage: Option<String>,
    
    /// Repository URL
    #[serde(default)]
    pub repository: Option<String>,
    
    /// Keywords for search
    #[serde(default)]
    pub keywords: Vec<String>,
    
    /// Categories
    #[serde(default)]
    pub categories: Vec<String>,
    
    /// Minimum NOVA compiler version required
    #[serde(default, rename = "nova-version")]
    pub nova_version: Option<String>,
}

/// Build configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Entry point for binaries
    #[serde(default = "default_entry")]
    pub entry: String,
    
    /// Output directory
    #[serde(default)]
    pub out_dir: Option<PathBuf>,
    
    /// Optimization level (0-3)
    #[serde(default)]
    pub opt_level: Option<u8>,
    
    /// Debug symbols
    #[serde(default)]
    pub debug: bool,
    
    /// Target triple
    #[serde(default)]
    pub target: Option<String>,
    
    /// Features to enable
    #[serde(default)]
    pub features: Vec<String>,
}

fn default_entry() -> String {
    "src/main.nova".to_string()
}

/// Dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencySpec {
    /// Simple version string: `package = "1.0"`
    Simple(String),
    
    /// Detailed specification
    Detailed(DependencyDetails),
}

/// Detailed dependency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDetails {
    /// Version requirement
    pub version: Option<String>,
    
    /// Git repository URL
    pub git: Option<String>,
    
    /// Git branch
    pub branch: Option<String>,
    
    /// Git tag
    pub tag: Option<String>,
    
    /// Git revision (commit hash)
    pub rev: Option<String>,
    
    /// Local path
    pub path: Option<PathBuf>,
    
    /// Registry URL
    pub registry: Option<String>,
    
    /// Features to enable
    #[serde(default)]
    pub features: Vec<String>,
    
    /// Use default features
    #[serde(default = "default_true")]
    pub default_features: bool,
    
    /// Mark as optional
    #[serde(default)]
    pub optional: bool,
}

fn default_true() -> bool {
    true
}

impl DependencySpec {
    /// Get the version requirement string
    pub fn version(&self) -> Option<&str> {
        match self {
            DependencySpec::Simple(v) => Some(v),
            DependencySpec::Detailed(d) => d.version.as_deref(),
        }
    }
    
    /// Check if this is a git dependency
    pub fn is_git(&self) -> bool {
        match self {
            DependencySpec::Simple(_) => false,
            DependencySpec::Detailed(d) => d.git.is_some(),
        }
    }
    
    /// Check if this is a path dependency
    pub fn is_path(&self) -> bool {
        match self {
            DependencySpec::Simple(_) => false,
            DependencySpec::Detailed(d) => d.path.is_some(),
        }
    }
}

/// Platform-specific dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetDependencies {
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySpec>,
}

/// Load manifest from a directory
pub fn load_manifest(dir: &Path) -> Result<Manifest, ManifestError> {
    let manifest_path = dir.join("Nova.toml");
    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = toml::from_str(&content)?;
    Ok(manifest)
}

/// Save manifest to a directory
pub fn save_manifest(dir: &Path, manifest: &Manifest) -> Result<(), ManifestError> {
    let manifest_path = dir.join("Nova.toml");
    let content = toml::to_string_pretty(manifest)?;
    std::fs::write(&manifest_path, content)?;
    Ok(())
}

/// Create a new manifest for a project
pub fn create_manifest(name: &str, is_lib: bool) -> Manifest {
    let entry = if is_lib {
        "src/lib.nova".to_string()
    } else {
        "src/main.nova".to_string()
    };
    
    Manifest {
        package: PackageInfo {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("A NOVA {}", if is_lib { "library" } else { "application" }),
            authors: vec![],
            license: None,
            homepage: None,
            repository: None,
            keywords: vec![],
            categories: vec![],
            nova_version: Some(">=0.3.0".to_string()),
        },
        build: BuildConfig {
            entry,
            out_dir: None,
            opt_level: None,
            debug: true,
            target: None,
            features: vec![],
        },
        dependencies: HashMap::new(),
        dev_dependencies: HashMap::new(),
        target: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_manifest_binary() {
        let manifest = create_manifest("my_app", false);
        assert_eq!(manifest.package.name, "my_app");
        assert_eq!(manifest.build.entry, "src/main.nova");
        assert!(manifest.dependencies.is_empty());
    }
    
    #[test]
    fn test_create_manifest_library() {
        let manifest = create_manifest("my_lib", true);
        assert_eq!(manifest.package.name, "my_lib");
        assert_eq!(manifest.build.entry, "src/lib.nova");
    }
    
    #[test]
    fn test_dependency_spec_simple() {
        let spec = DependencySpec::Simple("1.0".to_string());
        assert_eq!(spec.version(), Some("1.0"));
        assert!(!spec.is_git());
        assert!(!spec.is_path());
    }
    
    #[test]
    fn test_dependency_spec_detailed() {
        let spec = DependencySpec::Detailed(DependencyDetails {
            version: Some(">=1.0,<2.0".to_string()),
            git: Some("https://github.com/example/pkg.git".to_string()),
            branch: None,
            tag: None,
            rev: None,
            path: None,
            registry: None,
            features: vec![],
            default_features: true,
            optional: false,
        });
        assert!(spec.is_git());
        assert!(!spec.is_path());
    }
}
