//! Package manager commands implementation
//! 
//! Implements all CLI commands for the NOVA package manager.

use crate::manifest::{self, Manifest, DependencySpec, DependencyDetails};
use crate::resolver;
use crate::registry;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Manifest error: {0}")]
    Manifest(#[from] crate::manifest::ManifestError),
    
    #[error("Resolver error: {0}")]
    Resolver(#[from] crate::resolver::ResolverError),
    
    #[error("Registry error: {0}")]
    Registry(#[from] crate::registry::RegistryError),
    
    #[error("Project already exists: {0}")]
    ProjectExists(String),
    
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    
    #[error("Build failed: {0}")]
    BuildFailed(String),
    
    #[error("No Nova.toml found in current directory")]
    NoManifest,
}

/// Initialize a new NOVA project
pub fn init(name: &str, is_lib: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let project_dir = cwd.join(name);
    
    if project_dir.exists() {
        return Err(CommandError::ProjectExists(name.to_string()));
    }
    
    println!("Creating {} `{}`...", if is_lib { "library" } else { "binary" }, name);
    
    // Create directory structure
    std::fs::create_dir_all(&project_dir)?;
    std::fs::create_dir_all(project_dir.join("src"))?;
    
    // Create manifest
    let manifest = manifest::create_manifest(name, is_lib);
    manifest::save_manifest(&project_dir, &manifest)?;
    
    // Create source file
    let source_path = project_dir.join("src").join(if is_lib { "lib.nova" } else { "main.nova" });
    let source_content = if is_lib {
        format!(r#"// {} - A NOVA library

pub fn hello() -> String {{
    return "Hello from {}!";
}}
"#, name, name)
    } else {
        r#"// NOVA Application Entry Point

fn main() {
    print("Hello, NOVA!");
}
"#
    };
    std::fs::write(&source_path, source_content)?;
    
    // Create .gitignore
    let gitignore = r#"# Build artifacts
build/
dist/
*.o
*.so
*.dll
*.exe

# Dependencies
nova_modules/

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
"#;
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;
    
    // Create README
    let readme = format!(r#"# {}

{}

## Getting Started

```bash
nova-pkg build
nova-pkg run
```

## License

MIT
"#, name, manifest.package.description);
    std::fs::write(project_dir.join("README.md"), readme)?;
    
    println!("✓ Created {}", name);
    println!("  - Nova.toml");
    println!("  - src/{}", if is_lib { "lib.nova" } else { "main.nova" });
    println!("  - .gitignore");
    println!("  - README.md");
    println!("\nRun `cd {} && nova-pkg build` to get started!", name);
    
    Ok(())
}

/// Add a dependency to the project
pub fn add(name: &str, version: &str, is_dev: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let mut manifest = manifest::load_manifest(&cwd)?;
    
    // Check if dependency already exists
    let deps = if is_dev {
        &mut manifest.dev_dependencies
    } else {
        &mut manifest.dependencies
    };
    
    if deps.contains_key(name) {
        println!("Dependency `{}` already exists, updating...", name);
    }
    
    // Add or update dependency
    deps.insert(
        name.to_string(),
        DependencySpec::Simple(version.to_string()),
    );
    
    manifest::save_manifest(&cwd, &manifest)?;
    
    println!("✓ Added `{}` v{} to {}", name, version, if is_dev { "dev-dependencies" } else { "dependencies" });
    
    // Resolve and download dependencies
    resolve_dependencies()?;
    
    Ok(())
}

/// Remove a dependency from the project
pub fn remove(name: &str, is_dev: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let mut manifest = manifest::load_manifest(&cwd)?;
    
    let deps = if is_dev {
        &mut manifest.dev_dependencies
    } else {
        &mut manifest.dependencies
    };
    
    if !deps.contains_key(name) {
        return Err(CommandError::PackageNotFound(name.to_string()));
    }
    
    deps.remove(name);
    manifest::save_manifest(&cwd, &manifest)?;
    
    println!("✓ Removed `{}` from {}", name, if is_dev { "dev-dependencies" } else { "dependencies" });
    
    // Clean up nova_modules
    resolve_dependencies()?;
    
    Ok(())
}

/// Update dependencies
pub fn update(name: Option<&str>, breaking: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let mut manifest = manifest::load_manifest(&cwd)?;
    
    if let Some(pkg_name) = name {
        // Update specific package
        if let Some(dep) = manifest.dependencies.get_mut(pkg_name) {
            let latest = registry::get_latest_version(pkg_name)?;
            if breaking || semver_compatible(dep.version().unwrap_or("*"), &latest) {
                *dep = DependencySpec::Simple(latest.clone());
                println!("✓ Updated `{}` to v{}", pkg_name, latest);
            } else {
                println!("⚠ Skipping `{}`: v{} is incompatible with current requirement", 
                         pkg_name, latest);
            }
        }
    } else {
        // Update all dependencies
        let mut updated = Vec::new();
        for (pkg_name, dep) in manifest.dependencies.iter_mut() {
            if let Some(current) = dep.version() {
                let latest = match registry::get_latest_version(pkg_name) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if breaking || semver_compatible(current, &latest) {
                    *dep = DependencySpec::Simple(latest.clone());
                    updated.push((pkg_name.clone(), latest));
                }
            }
        }
        
        if updated.is_empty() {
            println!("All dependencies are up to date");
        } else {
            for (name, version) in updated {
                println!("✓ Updated `{}` to v{}", name, version);
            }
        }
    }
    
    manifest::save_manifest(&cwd, &manifest)?;
    resolve_dependencies()?;
    
    Ok(())
}

/// Build the project
pub fn build(release: bool, no_incremental: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load_manifest(&cwd)?;
    
    println!("Building {} v{}...", manifest.package.name, manifest.package.version);
    
    // First resolve dependencies
    resolve_dependencies()?;
    
    // Call the compiler
    let nova_compiler = std::env::var("NOVA_COMPILER")
        .unwrap_or_else(|_| "nova".to_string());
    
    let mut cmd = std::process::Command::new(&nova_compiler);
    cmd.arg("build")
       .arg("-i")
       .arg(manifest.build.entry);
    
    if release {
        cmd.arg("-r");
    }
    
    if no_incremental {
        cmd.arg("--no-incremental");
    }
    
    if let Some(out_dir) = &manifest.build.out_dir {
        cmd.arg("-o").arg(out_dir);
    }
    
    let output = cmd.output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CommandError::BuildFailed(stderr.to_string()));
    }
    
    println!("✓ Build completed successfully");
    
    if let Ok(stdout) = String::from_utf8(output.stdout) {
        if !stdout.is_empty() {
            print!("{}", stdout);
        }
    }
    
    Ok(())
}

/// Run the project
pub fn run(args: &[String], release: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load_manifest(&cwd)?;
    
    // Build first
    build(release, false)?;
    
    // Determine output binary location
    let bin_name = manifest.package.name.replace('-', "_");
    let profile = if release { "release" } else { "debug" };
    let out_dir = manifest.build.out_dir
        .unwrap_or_else(|| cwd.join("build").join(profile));
    let binary = out_dir.join(&bin_name);
    
    if !binary.exists() {
        return Err(CommandError::BuildFailed(
            format!("Binary not found at {:?}", binary)
        ));
    }
    
    println!("Running {}...", manifest.package.name);
    
    let status = std::process::Command::new(&binary)
        .args(args)
        .status()?;
    
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    
    Ok(())
}

/// Publish the package
pub fn publish(registry_url: Option<&str>, dry_run: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load_manifest(&cwd)?;
    
    println!("Publishing {} v{}...", manifest.package.name, manifest.package.version);
    
    if dry_run {
        println!("🔍 Dry run mode - no changes will be made");
        
        // Validate manifest
        if manifest.package.name.is_empty() {
            return Err(CommandError::Manifest(
                crate::manifest::ManifestError::MissingField("package.name".to_string())
            ));
        }
        
        // Check for required files
        let entry_path = cwd.join(&manifest.build.entry);
        if !entry_path.exists() {
            return Err(CommandError::BuildFailed(
                format!("Entry point not found: {:?}", entry_path)
            ));
        }
        
        println!("✓ Manifest validation passed");
        println!("✓ Entry point found: {:?}", entry_path);
        println!("\nWould publish to registry");
        return Ok(());
    }
    
    // Get registry
    let reg_url = registry_url.unwrap_or(registry::DEFAULT_REGISTRY);
    let reg = registry::Registry::new(reg_url)?;
    
    // Create package archive
    let archive = create_package_archive(&cwd, &manifest)?;
    
    // Publish
    reg.publish(&manifest, &archive)?;
    
    println!("✓ Published {} v{} to {}", 
             manifest.package.name, manifest.package.version, reg_url);
    
    Ok(())
}

/// Search for packages
pub fn search(query: &str, limit: usize) -> Result<(), CommandError> {
    println!("Searching for \"{}\"...", query);
    
    let results = registry::search(query, limit)?;
    
    if results.is_empty() {
        println!("No packages found matching \"{}\"", query);
        return Ok(());
    }
    
    println!("\nFound {} package(s):\n", results.len());
    
    for pkg in results {
        println!("{} v{}", pkg.name, pkg.version);
        if !pkg.description.is_empty() {
            println!("  {}", pkg.description);
        }
        println!("  Authors: {}", pkg.authors.join(", "));
        println!();
    }
    
    Ok(())
}

/// Show package information
pub fn info(name: &str, version: Option<&str>) -> Result<(), CommandError> {
    let pkg = if let Some(ver) = version {
        registry::get_package_version(name, ver)?
    } else {
        registry::get_latest_package(name)?
    };
    
    println!("{} v{}", pkg.name, pkg.version);
    println!();
    println!("Description: {}", pkg.description);
    println!("Authors: {}", pkg.authors.join(", "));
    
    if let Some(license) = &pkg.license {
        println!("License: {}", license);
    }
    
    if let Some(homepage) = &pkg.homepage {
        println!("Homepage: {}", homepage);
    }
    
    if let Some(repository) = &pkg.repository {
        println!("Repository: {}", repository);
    }
    
    if !pkg.keywords.is_empty() {
        println!("Keywords: {}", pkg.keywords.join(", "));
    }
    
    if !pkg.categories.is_empty() {
        println!("Categories: {}", pkg.categories.join(", "));
    }
    
    // Show dependencies
    if !pkg.dependencies.is_empty() {
        println!("\nDependencies:");
        for (dep_name, dep_spec) in &pkg.dependencies {
            if let Some(ver) = dep_spec.version() {
                println!("  {} {}", dep_name, ver);
            }
        }
    }
    
    Ok(())
}

/// Display dependency tree
pub fn tree(depth: Option<usize>) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load_manifest(&cwd)?;
    
    println!("Dependency tree for {} v{}\n", manifest.package.name, manifest.package.version);
    
    let resolved = resolver::resolve(&manifest)?;
    
    print_tree(&resolved, 0, depth.map(|d| d as i32).unwrap_or(-1), &mut Vec::new())?;
    
    Ok(())
}

fn print_tree(
    resolved: &HashMap<String, Manifest>,
    level: usize,
    max_depth: i32,
    visited: &mut Vec<String>,
) -> Result<(), CommandError> {
    if max_depth >= 0 && level as i32 > max_depth {
        return Ok(());
    }
    
    let indent = "│   ".repeat(level);
    
    for (i, (name, manifest)) in resolved.iter().enumerate() {
        let is_last = i == resolved.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        
        if visited.contains(&name) {
            println!("{}{}{} (*)", indent, connector, name);
        } else {
            println!("{}{}{} v{}", indent, connector, name, manifest.package.version);
            visited.push(name.clone());
            
            if !manifest.dependencies.is_empty() {
                let sub_resolved = resolver::resolve(manifest)?;
                print_tree(&sub_resolved, level + 1, max_depth, visited)?;
            }
        }
    }
    
    Ok(())
}

/// Clean build artifacts
pub fn clean(release_only: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load_manifest(&cwd)?;
    
    let build_dir = manifest.build.out_dir
        .unwrap_or_else(|| cwd.join("build"));
    
    if build_dir.exists() {
        if release_only {
            let release_dir = build_dir.join("release");
            if release_dir.exists() {
                std::fs::remove_dir_all(&release_dir)?;
                println!("✓ Cleaned release build");
            }
        } else {
            std::fs::remove_dir_all(&build_dir)?;
            println!("✓ Cleaned build directory");
        }
    }
    
    // Clean nova_modules
    let modules_dir = cwd.join("nova_modules");
    if modules_dir.exists() {
        std::fs::remove_dir_all(&modules_dir)?;
        println!("✓ Cleaned dependencies");
    }
    
    Ok(())
}

/// Check for outdated dependencies
pub fn outdated(major_only: bool) -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load_manifest(&cwd)?;
    
    println!("Checking for outdated dependencies...\n");
    
    let mut outdated = Vec::new();
    
    for (name, dep) in &manifest.dependencies {
        if let Some(current) = dep.version() {
            match registry::get_latest_version(name) {
                Ok(latest) => {
                    if latest != current {
                        if !major_only || is_major_update(current, &latest) {
                            outdated.push((name.clone(), current.to_string(), latest));
                        }
                    }
                }
                Err(_) => {
                    println!("⚠ Could not check updates for `{}`", name);
                }
            }
        }
    }
    
    if outdated.is_empty() {
        println!("✓ All dependencies are up to date!");
    } else {
        println!("Found {} outdated package(s):\n", outdated.len());
        println!("{:<20} {:<12} {:<12}", "Package", "Current", "Latest");
        println!("{:-<50}", "");
        for (name, current, latest) in outdated {
            println!("{:<20} {:<12} {:<12}", name, current, latest);
        }
        println!("\nRun `nova-pkg update` to update all compatible versions");
    }
    
    Ok(())
}

/// Helper functions

fn resolve_dependencies() -> Result<(), CommandError> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load_manifest(&cwd)?;
    
    print!("Resolving dependencies... ");
    io::stdout().flush()?;
    
    let resolved = resolver::resolve(&manifest)?;
    
    // Download and install dependencies
    let modules_dir = cwd.join("nova_modules");
    std::fs::create_dir_all(&modules_dir)?;
    
    for (name, dep_manifest) in &resolved {
        let dep_dir = modules_dir.join(name);
        if !dep_dir.exists() {
            std::fs::create_dir_all(&dep_dir)?;
            manifest::save_manifest(&dep_dir, dep_manifest)?;
        }
    }
    
    println!("✓ Resolved {} dependencies", resolved.len());
    
    Ok(())
}

fn semver_compatible(current: &str, latest: &str) -> bool {
    // Simple semver compatibility check
    // In production, use the semver crate properly
    let current_parts: Vec<u32> = current
        .trim_start_matches('^')
        .trim_start_matches('~')
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let latest_parts: Vec<u32> = latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    if current_parts.is_empty() || latest_parts.is_empty() {
        return true; // Be permissive if parsing fails
    }
    
    // Major version must match for compatibility
    current_parts[0] == latest_parts[0]
}

fn is_major_update(current: &str, latest: &str) -> bool {
    let current_parts: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let latest_parts: Vec<u32> = latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    if current_parts.is_empty() || latest_parts.is_empty() {
        return false;
    }
    
    current_parts[0] != latest_parts[0]
}

fn create_package_archive(dir: &Path, _manifest: &Manifest) -> Result<Vec<u8>, CommandError> {
    // In production, this would create a proper tarball
    // For now, just collect the manifest as a placeholder
    let content = format!(
        "Package archive for {}\nGenerated at: {:?}",
        dir.display(),
        std::time::SystemTime::now()
    );
    Ok(content.into_bytes())
}
