//! Reading `.debug_info` for local variables.
//!
//! ADR 0023. `nova-compiler` links with `-g`, so the C compiler writes local-variable DWARF into
//! every binary it builds. What was missing was a reader: this debugger has only ever parsed the
//! line table (through `addr2line`), and the hand-rolled model that used to sit here could never be
//! populated.
//!
//! This module does one job and says what it cannot do:
//!
//! - **it finds a function's variables by name**, with their declared type and the location
//!   expression the compiler chose for them;
//! - **it does not evaluate that expression** against a stopped process yet. A location is a small
//!   program (`DW_OP_fbreg -8`, or a register, or a composite), and running it needs the frame base
//!   from a live frame. That is the next step, and it is named here rather than implied by a
//!   function that looks finished.
//!
//! The read-only crates `gimli` and `object` do the format work. DWARF is a format where a partial
//! hand-written parser produces plausible *wrong* answers — a variable read from the wrong slot
//! looks like a number, not like an error — which is why this does not parse bytes itself.

use std::path::Path;

/// One local variable of a function, as the compiler described it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariable {
    /// The name as written in the source.
    pub name: String,
    /// The declared type, as DWARF spells it (`int`, `long long`, `char *`).
    pub type_name: String,
    /// The location expression, rendered for a human. Not evaluated: see the module docs.
    pub location: String,
}

/// What can go wrong reading a binary's debug information.
#[derive(Debug, thiserror::Error)]
pub enum DwarfError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} has no debug information: build it with `-g`")]
    NoDebugInfo { path: String },
    #[error("the debug information in {path} cannot be read: {message}")]
    Malformed { path: String, message: String },
    #[error("no function named `{name}` in {path}")]
    NoSuchFunction { name: String, path: String },
}

/// Every local variable of `function` in the binary at `path`.
///
/// `path` is the *binary*, not the source: the variables live in the executable's `.debug_info`.
pub fn locals_of_function(path: &Path, function: &str) -> Result<Vec<LocalVariable>, DwarfError> {
    let display = path.display().to_string();
    let data = std::fs::read(path).map_err(|source| DwarfError::Io {
        path: display.clone(),
        source,
    })?;

    let object = object::File::parse(&*data).map_err(|error| DwarfError::Malformed {
        path: display.clone(),
        message: error.to_string(),
    })?;

    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    let load_section = |id: gimli::SectionId| -> Result<std::borrow::Cow<[u8]>, DwarfError> {
        let bytes = object
            .section_by_name(id.name())
            .and_then(|section| section.data().ok())
            .unwrap_or(&[]);
        Ok(std::borrow::Cow::Owned(bytes.to_vec()))
    };
    let dwarf_cow = gimli::DwarfSections::load(&load_section).map_err(|error| {
        DwarfError::Malformed {
            path: display.clone(),
            message: error.to_string(),
        }
    })?;
    let dwarf = dwarf_cow.borrow(|section| gimli::EndianSlice::new(section, endian));

    // No `.debug_info` means no variables — say so rather than answer "none", which would read as
    // "this function has no locals".
    if dwarf.debug_info.units().count() == 0 {
        return Err(DwarfError::NoDebugInfo { path: display });
    }

    let mut units = dwarf.units();
    while let Some(header) = units.next().map_err(|error| DwarfError::Malformed {
        path: display.clone(),
        message: error.to_string(),
    })? {
        let unit = dwarf
            .unit(header)
            .map_err(|error| DwarfError::Malformed {
                path: display.clone(),
                message: error.to_string(),
            })?;

        if let Some(found) = locals_in_unit(&dwarf, &unit, function).map_err(|error| {
            DwarfError::Malformed {
                path: display.clone(),
                message: error.to_string(),
            }
        })? {
            return Ok(found);
        }
    }

    Err(DwarfError::NoSuchFunction {
        name: function.to_string(),
        path: display,
    })
}

/// The variables of `function` if this compile unit declares it.
///
/// `Ok(None)` means "not in this unit", which is not an error: a binary has one unit per translation
/// unit and the function is in exactly one of them.
fn locals_in_unit<R: gimli::Reader>(
    dwarf: &gimli::Dwarf<R>,
    unit: &gimli::Unit<R>,
    function: &str,
) -> Result<Option<Vec<LocalVariable>>, gimli::Error> {
    let mut entries = unit.entries();
    let mut depth: isize = 0;

    // Walk depth-first. A `DW_TAG_subprogram` whose name matches opens a scope; its children at the
    // next depth that are `DW_TAG_variable` are its locals.
    while let Some((delta, entry)) = entries.next_dfs()? {
        depth += delta;

        if entry.tag() != gimli::DW_TAG_subprogram {
            continue;
        }
        let name = entry
            .attr_value(gimli::DW_AT_name)?
            .and_then(|value| dwarf.attr_string(unit, value).ok())
            .and_then(|reader| reader.to_string().ok());
        if name.as_deref() != Some(function) {
            continue;
        }

        let subprogram_depth = depth;
        let mut locals = Vec::new();
        while let Some((delta, child)) = entries.next_dfs()? {
            depth += delta;
            if depth <= subprogram_depth {
                // The next sibling of the subprogram: its scope is over.
                break;
            }
            if child.tag() != gimli::DW_TAG_variable {
                continue;
            }
            let Some(name) = child
                .attr_value(gimli::DW_AT_name)?
                .and_then(|value| dwarf.attr_string(unit, value).ok())
                .and_then(|reader| reader.to_string().ok())
            else {
                continue;
            };
            let type_name = child
                .attr_value(gimli::DW_AT_type)?
                .and_then(|value| type_name_of(dwarf, unit, value).ok())
                .unwrap_or_else(|| "unknown".to_string());
            let location = child
                .attr_value(gimli::DW_AT_location)?
                .map(render_location)
                .unwrap_or_else(|| "no location (optimised away, or a global)".to_string());
            locals.push(LocalVariable {
                name,
                type_name,
                location,
            });
        }
        return Ok(Some(locals));
    }
    Ok(None)
}

/// The name a `DW_AT_type` points at, following references through the unit's DIE tree.
fn type_name_of<R: gimli::Reader>(
    dwarf: &gimli::Dwarf<R>,
    unit: &gimli::Unit<R>,
    value: gimli::AttributeValue<R>,
) -> Result<String, gimli::Error> {
    let offset = match value {
        gimli::AttributeValue::UnitRef(offset) => offset,
        _ => return Ok("unknown".to_string()),
    };
    let entry = dwarf.unit_entry(unit, offset)?;
    let name = entry
        .attr_value(gimli::DW_AT_name)?
        .and_then(|value| dwarf.attr_string(unit, value).ok())
        .and_then(|reader| reader.to_string().ok());
    Ok(name.unwrap_or_else(|| "unknown".to_string()))
}

/// A location expression, rendered for a human.
///
/// Rendered rather than evaluated: running it needs the frame base from a live process, which is the
/// step this module does not take.
fn render_location<R: gimli::Reader>(value: gimli::AttributeValue<R>) -> String {
    match value {
        gimli::AttributeValue::Exprloc(expression) => {
            let mut operations = expression.operations(Default::default());
            let mut parts = Vec::new();
            while let Ok(Some(operation)) = operations.next() {
                parts.push(format!("{:?}", operation));
            }
            if parts.is_empty() {
                "empty location expression".to_string()
            } else {
                parts.join(", ")
            }
        }
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A C program with two locals, built with `-g` by the host compiler.
    ///
    /// The test is on the *reader*, not on NOVA: whether a variable can be found at all is a
    /// question about DWARF, and answering it needs a binary that has some. Building one with `cc`
    /// keeps the test honest on any machine that can compile C, which is what the project requires
    /// of itself anyway.
    fn build_fixture() -> Option<std::path::PathBuf> {
        let dir = std::env::temp_dir().join(format!("nova-dwarf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        let source = dir.join("fixture.c");
        std::fs::write(
            &source,
            "#include <stdio.h>\n\
             int main(void) {\n\
                 int counter = 7;\n\
                 long long total = 11;\n\
                 printf(\"%d %lld\\n\", counter, total);\n\
                 return 0;\n\
             }\n",
        )
        .ok()?;
        let binary = dir.join("fixture");
        let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
        let output = std::process::Command::new(cc)
            .arg("-g")
            .arg("-O0")
            .arg("-o")
            .arg(&binary)
            .arg(&source)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(binary)
    }

    #[test]
    fn a_functions_locals_are_read_from_a_debug_build() {
        let Some(binary) = build_fixture() else {
            eprintln!("skipping: no host C compiler that can build with -g");
            return;
        };
        let locals = locals_of_function(&binary, "main").expect("main has locals");
        let names: Vec<&str> = locals.iter().map(|local| local.name.as_str()).collect();
        assert!(names.contains(&"counter"), "got {names:?}");
        assert!(names.contains(&"total"), "got {names:?}");

        let counter = locals
            .iter()
            .find(|local| local.name == "counter")
            .expect("counter");
        assert_eq!(counter.type_name, "int", "got {:?}", counter.type_name);
        assert!(
            !counter.location.is_empty(),
            "a local should carry a location expression"
        );
    }

    #[test]
    fn a_binary_without_debug_info_says_so_rather_than_answering_none() {
        // "No variables" and "no debug information" are different answers, and the difference is
        // the one a user acts on.
        let dir = std::env::temp_dir().join(format!("nova-nodwarf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let source = dir.join("plain.c");
        std::fs::write(&source, "int main(void) { return 0; }\n").expect("write");
        let binary = dir.join("plain");
        let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
        let output = std::process::Command::new(cc)
            .arg("-o")
            .arg(&binary)
            .arg(&source)
            .output();
        let Ok(output) = output else {
            eprintln!("skipping: no host C compiler");
            return;
        };
        if !output.status.success() {
            eprintln!("skipping: the host C compiler could not build the fixture");
            return;
        }
        let error = locals_of_function(&binary, "main").expect_err("no debug info");
        assert!(
            matches!(error, DwarfError::NoDebugInfo { .. }),
            "got {error}"
        );
    }
}
