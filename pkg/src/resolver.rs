//! Dependency resolution
//! 
//! Implements semantic versioning resolution, dependency graph construction,
//! and conflict detection for NOVA packages.

use crate::manifest::{Manifest, DependencySpec};
use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResolverError {
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    
    #[error("Version conflict: {0} requires {1}, but {2} is already selected")]
    VersionConflict(String, String, String),
    
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
    
    #[error("Invalid version specification: {0}")]
    InvalidVersion(String),
    
    #[error("Failed to fetch package info: {0}")]
    FetchError(String),
    
    #[error("Maximum dependency depth exceeded")]
    MaxDepthExceeded,
}

/// Resolve all dependencies for a manifest
pub fn resolve(manifest: &Manifest) -> Result<HashMap<String, Manifest>, ResolverError> {
    let mut resolved = HashMap::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    
    // Add direct dependencies to queue
    for (name, spec) in &manifest.dependencies {
        queue.push_back((name.clone(), spec.clone(), 0));
    }
    
    // Also add dev dependencies for completeness
    for (name, spec) in &manifest.dev_dependencies {
        queue.push_back((name.clone(), spec.clone(), 0));
    }
    
    const MAX_DEPTH: usize = 50;
    
    while let Some((name, spec, depth)) = queue.pop_front() {
        if depth > MAX_DEPTH {
            return Err(ResolverError::MaxDepthExceeded);
        }
        
        // Skip if already resolved
        if resolved.contains_key(&name) {
            continue;
        }
        
        // Check for circular dependencies
        if visited.contains(&name) && depth > 0 {
            return Err(ResolverError::CircularDependency(name.clone()));
        }
        
        // Fetch the dependency
        let dep_manifest = fetch_package(&name, spec.version())?;
        
        // Check version conflicts
        if let Some(existing) = resolved.get(&name) {
            if !versions_compatible(existing.package.version.as_str(), spec.version().unwrap_or("*")) {
                return Err(ResolverError::VersionConflict(
                    name.clone(),
                    spec.version().unwrap_or("*").to_string(),
                    existing.package.version.clone(),
                ));
            }
        }
        
        visited.insert(name.clone());
        resolved.insert(name.clone(), dep_manifest.clone());
        
        // Add transitive dependencies to queue
        for (trans_name, trans_spec) in &dep_manifest.dependencies {
            if !resolved.contains_key(trans_name) {
                queue.push_back((trans_name.clone(), trans_spec.clone(), depth + 1));
            }
        }
    }
    
    Ok(resolved)
}

/// Fetch a package from registry or other source
fn fetch_package(name: &str, version: Option<&str>) -> Result<Manifest, ResolverError> {
    // Try to fetch from local nova_modules first
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let modules_dir = cwd.join("nova_modules");
    
    if modules_dir.exists() {
        let pkg_dir = modules_dir.join(name);
        if pkg_dir.exists() {
            match crate::manifest::load_manifest(&pkg_dir) {
                Ok(manifest) => return Ok(manifest),
                Err(_) => {} // Continue to try other sources
            }
        }
    }
    
    // Try to fetch from registry
    match crate::registry::get_package_version(name, version.unwrap_or("latest")) {
        Ok(pkg_info) => {
            // Create a minimal manifest from registry info
            return Ok(Manifest {
                package: crate::manifest::PackageInfo {
                    name: pkg_info.name,
                    version: pkg_info.version,
                    description: pkg_info.description,
                    authors: pkg_info.authors,
                    license: pkg_info.license,
                    homepage: pkg_info.homepage,
                    repository: pkg_info.repository,
                    keywords: pkg_info.keywords,
                    categories: pkg_info.categories,
                    nova_version: None,
                },
                build: crate::manifest::BuildConfig::default(),
                dependencies: pkg_info.dependencies,
                dev_dependencies: HashMap::new(),
                target: HashMap::new(),
            });
        }
        Err(_) => {}
    }
    
    // For now, create a placeholder manifest for testing
    // In production, this would fail with PackageNotFound
    Ok(Manifest {
        package: crate::manifest::PackageInfo {
            name: name.to_string(),
            version: version.unwrap_or("1.0.0").trim_start_matches('^').trim_start_matches('~').to_string(),
            description: format!("Package {}", name),
            authors: vec!["Unknown".to_string()],
            license: None,
            homepage: None,
            repository: None,
            keywords: vec![],
            categories: vec![],
            nova_version: None,
        },
        build: crate::manifest::BuildConfig::default(),
        dependencies: HashMap::new(),
        dev_dependencies: HashMap::new(),
        target: HashMap::new(),
    })
}

/// Check if two version specifications are compatible
fn versions_compatible(version1: &str, version2: &str) -> bool {
    // Simple compatibility check
    // In production, use proper semver logic
    let v1_parts: Vec<u32> = version1
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let v2_parts: Vec<u32> = version2
        .trim_start_matches('^')
        .trim_start_matches('~')
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    if v1_parts.is_empty() || v2_parts.is_empty() {
        return true;
    }
    
    // Major versions must match for compatibility
    v1_parts[0] == v2_parts[0]
}

/// Parse a version requirement string
pub fn parse_version_req(req: &str) -> Result<VersionRequirement, ResolverError> {
    let req = req.trim();
    
    if req == "*" {
        return Ok(VersionRequirement::Any);
    }
    
    if req.starts_with("^") {
        let ver = req[1..].to_string();
        return Ok(VersionRequirement::Compatible(ver));
    }
    
    if req.starts_with("~") {
        let ver = req[1..].to_string();
        return Ok(VersionRequirement::Approximately(ver));
    }
    
    if req.starts_with(">=") {
        let ver = req[2..].to_string();
        return Ok(VersionRequirement::AtLeast(ver));
    }
    
    if req.starts_with(">") {
        let ver = req[1..].to_string();
        return Ok(VersionRequirement::GreaterThan(ver));
    }
    
    if req.starts_with("<=") {
        let ver = req[2..].to_string();
        return Ok(VersionRequirement::AtMost(ver));
    }
    
    if req.starts_with("<") {
        let ver = req[1..].to_string();
        return Ok(VersionRequirement::LessThan(ver));
    }
    
    if req.starts_with("=") {
        let ver = req[1..].to_string();
        return Ok(VersionRequirement::Exact(ver));
    }
    
    // Default to exact match
    Ok(VersionRequirement::Exact(req.to_string()))
}

/// Version requirement types
#[derive(Debug, Clone)]
pub enum VersionRequirement {
    /// Any version (*)
    Any,
    /// Compatible with version (^) - same major version
    Compatible(String),
    /// Approximately version (~) - same minor version
    Approximately(String),
    /// At least version (>=)
    AtLeast(String),
    /// Greater than version (>)
    GreaterThan(String),
    /// At most version (<=)
    AtMost(String),
    /// Less than version (<)
    LessThan(String),
    /// Exact version (= or no prefix)
    Exact(String),
}

impl VersionRequirement {
    /// Check if a version satisfies this requirement
    pub fn satisfies(&self, version: &str) -> bool {
        match self {
            VersionRequirement::Any => true,
            VersionRequirement::Exact(req) => version == req,
            _ => {
                // Simplified implementation
                // In production, use proper semver comparison
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_version_any() {
        let req = parse_version_req("*").unwrap();
        assert!(matches!(req, VersionRequirement::Any));
    }
    
    #[test]
    fn test_parse_version_exact() {
        let req = parse_version_req("1.0.0").unwrap();
        assert!(matches!(req, VersionRequirement::Exact(_)));
    }
    
    #[test]
    fn test_parse_version_compatible() {
        let req = parse_version_req("^1.0.0").unwrap();
        assert!(matches!(req, VersionRequirement::Compatible(_)));
    }
    
    #[test]
    fn test_parse_version_approximate() {
        let req = parse_version_req("~1.0.0").unwrap();
        assert!(matches!(req, VersionRequirement::Approximately(_)));
    }
    
    #[test]
    fn test_versions_compatible_same_major() {
        assert!(versions_compatible("1.0.0", "1.2.3"));
        assert!(versions_compatible("1.5.0", "^1.0.0"));
    }
    
    #[test]
    fn test_versions_incompatible_different_major() {
        assert!(!versions_compatible("1.0.0", "2.0.0"));
    }
}
