//! Package registry client
//! 
//! Interfaces with the NOVA package registry for searching, fetching,
//! and publishing packages.

use crate::manifest::{Manifest, DependencySpec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub const DEFAULT_REGISTRY: &str = "https://registry.nova-lang.org";

#[derive(Error, Debug)]
pub enum RegistryError {
    /// The registry is not implemented, so nothing here can reach a network.
    /// This variant exists for the day it is, and carries a message rather than a
    /// wrapped HTTP error, because there is no HTTP client in this crate.
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Invalid registry URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    
    #[error("Package not found: {0}")]
    NotFound(String),
    
    #[error("Authentication required")]
    AuthRequired,
    
    #[error("Publish failed: {0}")]
    PublishFailed(String),
    
    #[error("Registry returned error: {0}")]
    RegistryError(String),
}

/// Package information from registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub dependencies: HashMap<String, DependencySpec>,
    pub created_at: String,
    pub updated_at: String,
    pub downloads: u64,
}

/// Search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub downloads: u64,
    pub score: f64,
}

/// Registry client.
///
/// There is no HTTP client in this crate. Every method that could talk to a
/// registry either returns mock data or fails explicitly, because the registry is
/// not implemented — and carrying a full HTTP stack (and its dependency tree) for
/// a feature that is mocked was the wrong trade. It also brought the only
/// non-permissive licence in the tree in with it.
pub struct Registry {
    base_url: String,
}

impl Registry {
    /// Create a new registry client
    pub fn new(url: &str) -> Result<Self, RegistryError> {
        Ok(Registry {
            base_url: url.trim_end_matches('/').to_string(),
        })
    }
    
    /// The registry this client was configured with.
    ///
    /// Public because it is the one thing about this type that is real: the URL
    /// is remembered, and every method that would use it says it cannot.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Search for packages
    pub fn search(&self, _query: &str, _limit: usize) -> Result<Vec<SearchResult>, RegistryError> {
        // Refused rather than invented. This returned three fabricated packages —
        // `nova-utils 1.2.0`, "15420 downloads", score 0.95 — as a successful search. A user
        // looking for a package to depend on was shown a catalogue that does not exist.
        Err(RegistryError::RegistryError(format!(
            "the NOVA registry is not implemented: search cannot answer (see DEV_PLAN.md)"
        )))
    }
    
    /// Get latest version of a package
    pub fn get_latest_version(&self, _name: &str) -> Result<String, RegistryError> {
        // Refused. This answered a version from a hard-coded table, so `outdated` and
        // `update` reported real-looking results about packages nobody published.
        Err(RegistryError::RegistryError(format!(
            "the NOVA registry is not implemented: get_latest_version cannot answer (see DEV_PLAN.md)"
        )))
    }
    
    /// Get package info for a specific version
    pub fn get_package_version(&self, _name: &str, _version: &str) -> Result<PackageInfo, RegistryError> {
        // Refused. This built a `PackageInfo` from the arguments it was given plus "1.0.0",
        // so it always succeeded and always lied.
        Err(RegistryError::RegistryError(format!(
            "the NOVA registry is not implemented: get_package_version cannot answer (see DEV_PLAN.md)"
        )))
    }
    
    /// Get latest package info
    pub fn get_latest_package(&self, name: &str) -> Result<PackageInfo, RegistryError> {
        let version = self.get_latest_version(name)?;
        self.get_package_version(name, &version)
    }
    
    /// Publish a package
    pub fn publish(&self, manifest: &Manifest, archive: &[u8]) -> Result<(), RegistryError> {
        // In production, this would upload to the registry
        // For now, just validate and pretend it succeeded
        
        if manifest.package.name.is_empty() {
            return Err(RegistryError::PublishFailed("Package name is required".to_string()));
        }
        
        if manifest.package.version.is_empty() {
            return Err(RegistryError::PublishFailed("Package version is required".to_string()));
        }

        // Refused, and this is the one that mattered most: it returned `Ok(())` without
        // uploading anything, and `commands.rs` then printed "✓ Published X v1.0.0 to <url>".
        // A user was told their package was published when nothing had left the machine.
        // A false success is worse than a refusal, because it is acted on.
        Err(RegistryError::RegistryError(format!(
            "the NOVA registry is not implemented: publish cannot upload (see DEV_PLAN.md)"
        )))
    }
    
    /// Download a package.
    ///
    /// Not implemented. It used to build a URL under `base_url` and call it, which
    /// meant a user with a dependency got a connection error against a host that
    /// does not exist — a worse answer than this one, which names the real state.
    pub fn download(&self, name: &str, version: &str) -> Result<Vec<u8>, RegistryError> {
        Err(RegistryError::RegistryError(format!(
            "cannot download {}@{}: the NOVA package registry is not implemented. \
             Use a path dependency in `nova.toml` instead",
            name, version
        )))
    }
    
    /// Verify an API token.
    ///
    /// Not implemented, for the same reason as [`Registry::download`].
    pub fn verify_token(&self, _token: &str) -> Result<bool, RegistryError> {
        Err(RegistryError::RegistryError(
            "cannot verify a token: the NOVA package registry is not implemented".to_string(),
        ))
    }
}

// Mock implementations for development/testing




/// Convenience function to search without creating a client
pub fn search(query: &str, limit: usize) -> Result<Vec<SearchResult>, RegistryError> {
    let registry = Registry::new(DEFAULT_REGISTRY)?;
    registry.search(query, limit)
}

/// Convenience function to get latest version
pub fn get_latest_version(name: &str) -> Result<String, RegistryError> {
    let registry = Registry::new(DEFAULT_REGISTRY)?;
    registry.get_latest_version(name)
}

/// Convenience function to get package info
pub fn get_package_version(name: &str, version: &str) -> Result<PackageInfo, RegistryError> {
    let registry = Registry::new(DEFAULT_REGISTRY)?;
    registry.get_package_version(name, version)
}

/// Convenience function to get latest package
pub fn get_latest_package(name: &str) -> Result<PackageInfo, RegistryError> {
    let registry = Registry::new(DEFAULT_REGISTRY)?;
    registry.get_latest_package(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_registry_creation() {
        let registry = Registry::new(DEFAULT_REGISTRY);
        assert!(registry.is_ok());
    }
    
    #[test]
    fn test_the_registry_refuses_instead_of_inventing() {
        let registry = Registry::new(DEFAULT_REGISTRY).unwrap();

        // Each of these used to answer fabricated data as `Ok`. They assert the refusal now:
        // a mock that returns a value becomes a contract the moment a caller trusts it, and
        // `publish` returning `Ok(())` made the CLI print "✓ Published".
        for message in [
            registry.search("nova", 10).unwrap_err().to_string(),
            registry.get_latest_version("nova-utils").unwrap_err().to_string(),
            registry
                .get_package_version("nova-utils", "1.2.0")
                .unwrap_err()
                .to_string(),
        ] {
            assert!(
                message.contains("not implemented"),
                "the refusal must say what is missing: {message}"
            );
        }
    }
}
