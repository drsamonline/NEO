//! Module loading: resolve `use name;` / `use name.*;` to files on disk and
//! merge their declarations into the program being compiled.
//!
//! Resolution is deliberately simple. For a module named `name`, the loader
//! looks beside the importing file (`name.nova`, `name/mod.nova`,
//! `name/name.nova`), then in every **module root** the caller supplied, then in
//! `stdlib/name/name.nova` in the source directory or any ancestor. A `use` that
//! resolves to no file is left alone: the analyser still sees it and name
//! resolution fails with the usual diagnostic.
//!
//! Module roots are how a dependency's source becomes importable. `nova-pkg` reads
//! `nova.toml`, materialises each dependency, and passes its directory as
//! `--module-root`; the compiler never reads a manifest and never fetches anything.

use crate::{Lexer, Parser};
use nova_common::{Item, Program};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Load every module the program (transitively) uses and return one merged
/// program: imported items first, then the original program's own items.
pub fn load_modules(
    program: Program,
    source: &Path,
    module_roots: &[PathBuf],
) -> Result<Program, String> {
    let mut loader = ModuleLoader {
        loaded: HashSet::new(),
        declared: HashSet::new(),
        items: Vec::new(),
        module_roots,
    };
    loader.collect_uses(&program, source)?;

    let mut items = loader.items;
    items.extend(program.items);
    Ok(Program {
        items,
        span: program.span,
    })
}

struct ModuleLoader<'a> {
    /// Canonical paths already loaded, so `core` imported twice is one module.
    loaded: HashSet<PathBuf>,
    /// Declaration names already merged, so two modules cannot emit two C
    /// functions with the same name.
    declared: HashSet<String>,
    items: Vec<Item>,
    /// Directories to search for modules, in order, after the importing file's own
    /// directory. A dependency's source root goes here.
    module_roots: &'a [PathBuf],
}

impl ModuleLoader {
    fn collect_uses(&mut self, program: &Program, source: &Path) -> Result<(), String> {
        for item in &program.items {
            let name = match item {
                Item::Use(use_decl) => use_decl.path.first().cloned(),
                // `mod name;` loads the same file a `use name;` would.
                Item::Mod(module) => Some(module.name.clone()),
                _ => None,
            };
            if let Some(name) = name {
                if let Some(path) = find_module(source, &name, self.module_roots) {
                    self.load(&path)?;
                }
            }
        }
        Ok(())
    }

    fn load(&mut self, path: &Path) -> Result<(), String> {
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if !self.loaded.insert(canonical) {
            return Ok(());
        }

        let source = fs::read_to_string(path)
            .map_err(|error| format!("cannot read module {}: {error}", path.display()))?;
        let mut lexer = Lexer::new(&source);
        let (tokens, diagnostics) = lexer.tokenize();
        if diagnostics.has_errors() {
            let messages: Vec<String> = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect();
            return Err(format!(
                "{}: lexical errors: {}",
                path.display(),
                messages.join("; ")
            ));
        }

        let mut parser = Parser::new(tokens);
        let program = parser.parse().map_err(|diagnostics| {
            let messages: Vec<String> = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect();
            format!(
                "{}: parse errors: {}",
                path.display(),
                messages.join("; ")
            )
        })?;

        // Depth-first: a module's own imports precede its declarations.
        self.collect_uses(&program, path)?;

        for item in program.items {
            if matches!(item, Item::Use(_)) {
                continue;
            }
            if let Some(name) = item_name(&item) {
                if !self.declared.insert(name) {
                    continue;
                }
            }
            self.items.push(item);
        }
        Ok(())
    }
}

/// The name a declaration introduces, for duplicate suppression.
fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Function(function) => Some(format!("fn {}", function.name)),
        Item::Struct(struct_def) => Some(format!("struct {}", struct_def.name)),
        Item::Enum(enum_def) => Some(format!("enum {}", enum_def.name)),
        Item::Const(const_def) => Some(format!("const {}", const_def.name)),
        Item::Impl(_) | Item::Use(_) | Item::Mod(_) | Item::TypeAlias(_) => None,
    }
}

/// Find the file for a module name, relative to the importing file.
fn find_module(source: &Path, name: &str, module_roots: &[PathBuf]) -> Option<PathBuf> {
    let directory = source.parent().unwrap_or_else(|| Path::new("."));
    let local = [
        directory.join(format!("{name}.nova")),
        directory.join(name).join("mod.nova"),
        directory.join(name).join(format!("{name}.nova")),
    ];
    for candidate in local {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // A dependency's source root, as handed over by `nova-pkg`. Searched before
    // `stdlib` so a project can shadow a stdlib module with its own, which is the
    // behaviour every other package manager has.
    for root in module_roots {
        let candidates = [
            root.join(format!("{name}.nova")),
            root.join(name).join("mod.nova"),
            root.join(name).join(format!("{name}.nova")),
            root.join("src").join(format!("{name}.nova")),
            root.join("src").join(name).join(format!("{name}.nova")),
        ];
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    for ancestor in directory.ancestors() {
        let candidate = ancestor
            .join("stdlib")
            .join(name)
            .join(format!("{name}.nova"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_resolves_beside_the_importing_file() {
        let root = std::env::temp_dir().join(format!("nova-modules-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        let main = root.join("main.nova");
        fs::write(&main, "use helper.*;\n").expect("write main");
        fs::write(root.join("helper.nova"), "fn helper() -> int { 1 }\n").expect("write helper");

        let module = find_module(&main, "helper", &[]).expect("helper should resolve");
        assert_eq!(module, root.join("helper.nova"));
    }

    #[test]
    fn a_module_root_is_searched_after_the_importing_directory() {
        let root = std::env::temp_dir().join(format!("nova-roots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let project = root.join("project");
        let dependency = root.join("dependency").join("src");
        fs::create_dir_all(&project).expect("project dir");
        fs::create_dir_all(&dependency).expect("dependency dir");

        let main = project.join("main.nova");
        fs::write(&main, "use utils;\n").expect("write main");
        fs::write(
            dependency.join("utils.nova"),
            "fn helper() -> int {\n    return 1;\n}\n",
        )
        .expect("write dependency module");

        // Not resolvable without the root, resolvable with it.
        assert!(find_module(&main, "utils", &[]).is_none());
        let found = find_module(&main, "utils", &[root.join("dependency")])
            .expect("a module root must be searched");
        assert!(found.ends_with("src/utils.nova"), "found {}", found.display());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_module_root_wins_over_stdlib() {
        // A project may shadow a stdlib module with its own, which is what every
        // other package manager does.
        let root = std::env::temp_dir().join(format!("nova-shadow-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let project = root.join("project");
        let dependency = root.join("dependency");
        fs::create_dir_all(&project).expect("project dir");
        fs::create_dir_all(&dependency).expect("dependency dir");

        let main = project.join("main.nova");
        fs::write(&main, "use core;\n").expect("write main");
        fs::write(
            dependency.join("core.nova"),
            "fn mine() -> int {\n    return 1;\n}\n",
        )
        .expect("write shadowing module");

        let found = find_module(&main, "core", &[dependency]).expect("resolves");
        assert!(found.ends_with("core.nova"), "found {}", found.display());
        assert!(
            !found.to_string_lossy().contains("stdlib"),
            "the module root must be searched before stdlib: {}",
            found.display()
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_stdlib_is_found_from_a_nested_module() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("compiler crate has a parent");
        let io = root.join("stdlib").join("io").join("io.nova");
        let core = find_module(&io, "core", &[]).expect("core should resolve");
        assert_eq!(core, root.join("stdlib").join("core").join("core.nova"));
    }
}
