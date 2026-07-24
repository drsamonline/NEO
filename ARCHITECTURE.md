# Compiler Architecture

Lexer -> Parser -> AST -> Semantic -> SIR -> Backend

## Repository Workspace

The repository root owns the Rust workspace. Each executable or library
component remains an independent crate:

- `compiler` (`nova-compiler`)
- `runtime` (`nova-runtime`)
- `package-manager` (`nova-package-manager`)
- `language-server` (`nova-language-server`)
- `debugger` (`nova-debugger`)
- `crates/nova-common` (`nova-common`)

`nova-common` is reserved for stable, shared contracts. It must not contain
component-specific implementation logic. Component crates remain independent
until a documented architectural dependency is required.
