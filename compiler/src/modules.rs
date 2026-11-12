//! Module loading: resolve `use name;` / `use name.*;` to files on disk and
//! merge their declarations into the program being compiled.
//!
//! Resolution is deliberately simple. For a module named `name`, the loader
//! looks beside the importing file (`name.nova`, `name/mod.nova`,
//! `name/name.nova`), then in `stdlib/name/name.nova` in the source directory or
//! any ancestor. A `use` that resolves to no file is left alone: the analyser
//! still sees it and name resolution fails with the usual diagnostic.

use crate::{Lexer, Parser};
use nova_common::{Item, Program};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Load every module the program (transitively) uses and return one merged
/// program: imported items first, then the original program's own items.
pub fn load_modules(program: Program, source: &Path) -> Result<Program, String> {
    let mut loader = ModuleLoader {
        loaded: HashSet::new(),
        declared: HashSet::new(),
        items: Vec::new(),
    };
    loader.collect_uses(&program, source)?;

    let mut items = loader.items;
    items.extend(program.items);
    Ok(Program {
        items,
        span: program.span,
    })
}

struct ModuleLoader {
    /// Canonical paths already loaded, so `core` imported twice is one module.
    loaded: HashSet<PathBuf>,
    /// Declaration names already merged, so two modules cannot emit two C
    /// functions with the same name.
    declared: HashSet<String>,
    items: Vec<Item>,
}

impl ModuleLoader {
    fn collect_uses(&mut self, program: &Program, source: &Path) -> Result<(), String> {
        for item in &program.items {
            let Item::Use(use_decl) = item else {
                continue;
            };
            let Some(name) = use_decl.path.first() else {
                continue;
            };
            if let Some(path) = find_module(source, name) {
                self.load(&path)?;
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
        Item::Impl(_) | Item::Use(_) | Item::Mod(_) | Item::TypeAlias(_) | Item::Trait(_) => None,
    }
}

/// Find the file for a module name, relative to the importing file.
fn find_module(source: &Path, name: &str) -> Option<PathBuf> {
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

        let module = find_module(&main, "helper").expect("helper should resolve");
        assert_eq!(module, root.join("helper.nova"));
    }

    #[test]
    fn the_stdlib_is_found_from_a_nested_module() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("compiler crate has a parent");
        let io = root.join("stdlib").join("io").join("io.nova");
        let core = find_module(&io, "core").expect("core should resolve");
        assert_eq!(core, root.join("stdlib").join("core").join("core.nova"));
    }
}
