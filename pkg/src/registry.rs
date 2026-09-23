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
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
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

/// Registry client
pub struct Registry {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl Registry {
    /// Create a new registry client
    pub fn new(url: &str) -> Result<Self, RegistryError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(format!("nova-pkg/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        
        Ok(Registry {
            base_url: url.trim_end_matches('/').to_string(),
            client,
        })
    }
    
    /// Search for packages
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, RegistryError> {
        // For now, return mock results since we don't have a real registry
        Ok(mock_search(query, limit))
    }
    
    /// Get latest version of a package
    pub fn get_latest_version(&self, name: &str) -> Result<String, RegistryError> {
        // For now, return mock data
        Ok(mock_get_latest_version(name))
    }
    
    /// Get package info for a specific version
    pub fn get_package_version(&self, name: &str, version: &str) -> Result<PackageInfo, RegistryError> {
        // For now, return mock data
        Ok(mock_get_package(name, version))
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
        
        // Mock success
        Ok(())
    }
    
    /// Download a package
    pub fn download(&self, name: &str, version: &str) -> Result<Vec<u8>, RegistryError> {
        let url = format!("{}/packages/{}/{}/download", self.base_url, name, version);
        
        let response = self.client.get(&url).send()?;
        
        if !response.status().is_success() {
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Err(RegistryError::NotFound(format!("{}@{}", name, version)));
            }
            return Err(RegistryError::RegistryError(format!(
                "HTTP {}",
                response.status()
            )));
        }
        
        let bytes = response.bytes()?;
        Ok(bytes.to_vec())
    }
    
    /// Verify API token
    pub fn verify_token(&self, token: &str) -> Result<bool, RegistryError> {
        let url = format!("{}/api/verify", self.base_url);
        
        let response = self.client
            .get(&url)
            .bearer_auth(token)
            .send()?;
        
        Ok(response.status().is_success())
    }
}

// Mock implementations for development/testing

fn mock_search(query: &str, limit: usize) -> Vec<SearchResult> {
    let mock_packages = vec![
        SearchResult {
            name: "nova-utils".to_string(),
            version: "1.2.0".to_string(),
            description: "Common utilities for NOVA development".to_string(),
            authors: vec!["NOVA Team".to_string()],
            downloads: 15420,
            score: 0.95,
        },
        SearchResult {
            name: "nova-json".to_string(),
            version: "0.8.3".to_string(),
            description: "JSON parsing and serialization for NOVA".to_string(),
            authors: vec!["NOVA Team".to_string()],
            downloads: 8932,
            score: 0.87,
        },
        SearchResult {
            name: "nova-http".to_string(),
            version: "2.1.0".to_string(),
            description: "HTTP client and server library".to_string(),
            authors: vec!["NOVA Team".to_string()],
            downloads: 5621,
            score: 0.82,
        },
        SearchResult {
            name: "nova-crypto".to_string(),
            version: "0.5.1".to_string(),
            description: "Cryptographic primitives for NOVA".to_string(),
            authors: vec!["Security Team".to_string()],
            downloads: 3245,
            score: 0.75,
        },
    ];
    
    let query_lower = query.to_lowercase();
    let mut results: Vec<_> = mock_packages
        .into_iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&query_lower) ||
            p.description.to_lowercase().contains(&query_lower)
        })
        .collect();
    
    results.truncate(limit);
    results
}

fn mock_get_latest_version(name: &str) -> String {
    match name {
        "nova-utils" => "1.2.0".to_string(),
        "nova-json" => "0.8.3".to_string(),
        "nova-http" => "2.1.0".to_string(),
        "nova-crypto" => "0.5.1".to_string(),
        _ => "1.0.0".to_string(),
    }
}

fn mock_get_package(name: &str, version: &str) -> PackageInfo {
    PackageInfo {
        name: name.to_string(),
        version: version.to_string(),
        description: format!("Mock package {}", name),
        authors: vec!["NOVA Team".to_string()],
        license: Some("MIT".to_string()),
        homepage: Some(format!("https://github.com/nova-lang/{}", name)),
        repository: Some(format!("https://github.com/nova-lang/{}", name)),
        keywords: vec!["nova".to_string(), "package".to_string()],
        categories: vec!["Development".to_string()],
        dependencies: HashMap::new(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-15T00:00:00Z".to_string(),
        downloads: 1000,
    }
}

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
    fn test_mock_search() {
        let results = mock_search("nova", 10);
        assert!(!results.is_empty());
    }
    
    #[test]
    fn test_mock_get_version() {
        let version = mock_get_latest_version("nova-utils");
        assert_eq!(version, "1.2.0");
    }
}
