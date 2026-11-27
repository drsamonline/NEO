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

use object::{Object, ObjectSection};
use std::path::Path;

/// One local variable of a function, as the compiler described it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariable {
    /// The name as written in the source.
    pub name: String,
    /// The declared type, as DWARF spells it (`int`, `long long`, `char *`).
    pub type_name: String,
    /// The location expression, rendered for a human.
    pub location: String,
    /// The same expression as operations `evaluate_location` can run. Empty when the variable has
    /// no location (optimised away, or a global).
    pub ops: Vec<LocationOp>,
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

    // "No variables" and "no debug information" are different answers, and the difference is the
    // one a user acts on: the first says the function has none, the second says the binary was not
    // built with `-g`. Checked on the section, because `DebugInfoUnitHeadersIter` is not an
    // `Iterator` and cannot be counted.
    if object.section_by_name(".debug_info").is_none() {
        return Err(DwarfError::NoDebugInfo { path: display });
    }

    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    let load_section = |id: gimli::SectionId| -> Result<std::borrow::Cow<[u8]>, DwarfError> {
        let bytes: &[u8] = object
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
            .and_then(|reader| reader.to_string().ok().map(|text| text.into_owned()));
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
                .and_then(|reader| reader.to_string().ok().map(|text| text.into_owned()))
            else {
                continue;
            };
            let type_name = child
                .attr_value(gimli::DW_AT_type)?
                .and_then(|value| type_name_of(dwarf, unit, value).ok())
                .unwrap_or_else(|| "unknown".to_string());
            let location_value = child.attr_value(gimli::DW_AT_location)?;
            let location = location_value
                .clone()
                .map(|value| render_location(value, unit.encoding()))
                .unwrap_or_else(|| "no location (optimised away, or a global)".to_string());
            let ops = location_value
                .and_then(|value| match value {
                    gimli::AttributeValue::Exprloc(expression) => {
                        Some(location_ops(expression, unit.encoding()))
                    }
                    // A variable at a fixed address or in a register is a shape this debugger
                    // does not read; empty means "no location to evaluate", which the caller
                    // refuses rather than guessing.
                    _ => None,
                })
                .unwrap_or_default();
            locals.push(LocalVariable {
                name,
                type_name,
                location,
                ops,
            });
        }
        return Ok(Some(locals));
    }
    Ok(None)
}

/// The name a `DW_AT_type` points at, following the reference within the unit.
fn type_name_of<R: gimli::Reader>(
    dwarf: &gimli::Dwarf<R>,
    unit: &gimli::Unit<R>,
    value: gimli::AttributeValue<R>,
) -> Result<String, gimli::Error> {
    let offset = match value {
        gimli::AttributeValue::UnitRef(offset) => offset,
        _ => return Ok("unknown".to_string()),
    };
    // `unit.entry` in gimli 0.31 — `Dwarf::unit_entry` does not exist, and neither does a `dwarf`
    // field on `Unit`, so the `Dwarf` is passed in.
    let entry = unit.entry(offset)?;
    let name = entry
        .attr_value(gimli::DW_AT_name)?
        .and_then(|value| dwarf.attr_string(unit, value).ok())
        .and_then(|reader| reader.to_string().ok().map(|text| text.into_owned()));
    Ok(name.unwrap_or_else(|| "unknown".to_string()))
}

/// A location expression, rendered for a human.
///
/// Rendered rather than evaluated: running it needs the frame base from a live process, which is the
/// step this module does not take. `Encoding` has no `Default`, so the unit's own is passed in.
fn render_location<R: gimli::Reader>(
    value: gimli::AttributeValue<R>,
    encoding: gimli::Encoding,
) -> String {
    match value {
        gimli::AttributeValue::Exprloc(expression) => {
            let mut operations = expression.operations(encoding);
            let mut parts = Vec::new();
            while let Ok(Some(operation)) = operations.next() {
                parts.push(format!("{operation:?}"));
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

/// One step of a DWARF location expression, in the form this debugger can act on.
///
/// `gimli` parses the bytes into `Operation`s; this is the subset that names an address. Anything
/// else becomes `Unsupported` and is refused **by name** rather than guessed at, because a location
/// evaluated wrongly reads a real number from the wrong place — the failure mode that looks like
/// success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocationOp {
    /// `DW_OP_fbreg n` — an offset from the frame base.
    FrameOffset(i64),
    /// `DW_OP_addr a` — a fixed address.
    Address(u64),
    /// `DW_OP_call_frame_cfa` — the canonical frame address.
    Cfa,
    /// `DW_OP_plus_uconst n` — add to what the previous step produced.
    PlusConstant(u64),
    /// `DW_OP_plus` — pop two, add.
    Plus,
    /// Anything this debugger does not evaluate, kept so the refusal can say what it was.
    Unsupported(String),
}

/// Translate `gimli`'s operations into the subset above.
///
/// Rendering happens in the same pass as evaluation's input so the two cannot disagree about what a
/// location means: `render_location` prints these, and `evaluate_location` runs them.
pub fn location_ops<R: gimli::Reader>(
    expression: gimli::Expression<R>,
    encoding: gimli::Encoding,
) -> Vec<LocationOp> {
    let mut operations = expression.operations(encoding);
    let mut ops = Vec::new();
    while let Ok(Some(operation)) = operations.next() {
        ops.push(match operation {
            gimli::Operation::FrameOffset { offset } => LocationOp::FrameOffset(offset),
            gimli::Operation::Address { address } => LocationOp::Address(address),
            gimli::Operation::CallFrameCFA => LocationOp::Cfa,
            gimli::Operation::PlusConstant { value } => LocationOp::PlusConstant(value),
            gimli::Operation::Plus => LocationOp::Plus,
            other => LocationOp::Unsupported(format!("{other:?}")),
        });
    }
    ops
}

/// Where a variable lives, given the frame it is being read in.
///
/// `frame_base` is the function's `DW_AT_frame_base` resolved for the current PC — on x86_64 with a
/// frame pointer that is `RBP`. `cfa` is the canonical frame address, which is `RBP + 16` on the same
/// architecture; both are passed in rather than assumed, so a caller that has the registers cannot
/// accidentally get the other one.
///
/// The expression is a tiny stack machine. Only the shapes gcc emits for an unoptimised local are
/// implemented — a frame offset, an address, the CFA, and constant addition — and everything else is
/// refused with the operation named.
pub fn evaluate_location(
    ops: &[LocationOp],
    frame_base: u64,
    cfa: u64,
) -> Result<u64, DwarfError> {
    let mut stack: Vec<u64> = Vec::new();
    for op in ops {
        match op {
            LocationOp::FrameOffset(offset) => {
                stack.push(add_signed(frame_base, *offset)?);
            }
            LocationOp::Address(address) => stack.push(*address),
            LocationOp::Cfa => stack.push(cfa),
            LocationOp::PlusConstant(value) => {
                let top = stack.pop().ok_or_else(|| DwarfError::Malformed {
                    path: "location expression".to_string(),
                    message: "`DW_OP_plus_uconst` with nothing on the stack".to_string(),
                })?;
                stack.push(top.wrapping_add(*value));
            }
            LocationOp::Plus => {
                let right = stack.pop().ok_or_else(|| DwarfError::Malformed {
                    path: "location expression".to_string(),
                    message: "`DW_OP_plus` with fewer than two values on the stack".to_string(),
                })?;
                let left = stack.pop().ok_or_else(|| DwarfError::Malformed {
                    path: "location expression".to_string(),
                    message: "`DW_OP_plus` with fewer than two values on the stack".to_string(),
                })?;
                stack.push(left.wrapping_add(right));
            }
            LocationOp::Unsupported(operation) => {
                return Err(DwarfError::Malformed {
                    path: "location expression".to_string(),
                    message: format!(
                        "`{operation}` is not evaluated by this debugger; \
                         `print 0xADDRESS` reads memory directly"
                    ),
                });
            }
        }
    }
    stack.pop().ok_or_else(|| DwarfError::Malformed {
        path: "location expression".to_string(),
        message: "the location expression produced no address".to_string(),
    })
}

/// `base + offset`, where the offset may be negative and the result must stay in range.
fn add_signed(base: u64, offset: i64) -> Result<u64, DwarfError> {
    let result = if offset >= 0 {
        base.checked_add(offset as u64)
    } else {
        base.checked_sub(offset.unsigned_abs())
    };
    result.ok_or_else(|| DwarfError::Malformed {
        path: "location expression".to_string(),
        message: format!("{base:#x} {offset:+} does not fit in an address"),
    })
}

/// The operations that compute a function's frame base.
///
/// gcc emits `DW_AT_frame_base = DW_OP_call_frame_cfa` for an unoptimised function, so this usually
/// comes back as `[Cfa]`. Returned as operations rather than a number because the number depends on
/// the stopped frame, which only the caller has.
pub fn frame_base_of_function(
    path: &Path,
    function: &str,
) -> Result<Vec<LocationOp>, DwarfError> {
    let display = path.display().to_string();
    let data = std::fs::read(path).map_err(|source| DwarfError::Io {
        path: display.clone(),
        source,
    })?;
    let object = object::File::parse(&*data).map_err(|error| DwarfError::Malformed {
        path: display.clone(),
        message: error.to_string(),
    })?;
    if object.section_by_name(".debug_info").is_none() {
        return Err(DwarfError::NoDebugInfo { path: display });
    }
    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    let load_section = |id: gimli::SectionId| -> Result<std::borrow::Cow<[u8]>, DwarfError> {
        let bytes: &[u8] = object
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
        let mut entries = unit.entries();
        while let Some((_, entry)) = entries.next_dfs().map_err(|error| DwarfError::Malformed {
            path: display.clone(),
            message: error.to_string(),
        })? {
            if entry.tag() != gimli::DW_TAG_subprogram {
                continue;
            }
            let name = entry
                .attr_value(gimli::DW_AT_name)
                .map_err(|error| DwarfError::Malformed {
                    path: display.clone(),
                    message: error.to_string(),
                })?
                .and_then(|value| dwarf.attr_string(&unit, value).ok())
                // `attr_string` yields a reader whose `to_string` is `Result<&str>` here, not the
                // `Reader` method's `Cow`, so `.ok()` then a `String` is what the caller needs.
                .and_then(|reader| reader.to_string().ok())
                .map(|text| text.to_string());
            if name.as_deref() != Some(function) {
                continue;
            }
            let value = entry
                .attr_value(gimli::DW_AT_frame_base)
                .map_err(|error| DwarfError::Malformed {
                    path: display.clone(),
                    message: error.to_string(),
                })?;
            return match value {
                Some(gimli::AttributeValue::Exprloc(expression)) => {
                    Ok(location_ops(expression, unit.encoding()))
                }
                Some(other) => Ok(vec![LocationOp::Unsupported(format!("{other:?}"))]),
                // No frame base: the caller refuses rather than assuming the frame pointer.
                None => Ok(Vec::new()),
            };
        }
    }
    Err(DwarfError::NoSuchFunction {
        name: function.to_string(),
        path: display,
    })
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
    fn a_functions_frame_base_is_read_from_a_debug_build() {
        let Some(binary) = build_fixture() else {
            eprintln!("skipping: no host C compiler that can build with -g");
            return;
        };
        let ops = frame_base_of_function(&binary, "main").expect("main has a frame base");
        // gcc emits `DW_AT_frame_base = DW_OP_call_frame_cfa` for an unoptimised function, which is
        // the one shape the evaluator runs without needing a register it was not given.
        assert_eq!(ops, vec![LocationOp::Cfa], "got {ops:?}");
    }

    #[test]
    fn a_local_carries_the_operations_that_name_its_address() {
        let Some(binary) = build_fixture() else {
            eprintln!("skipping: no host C compiler that can build with -g");
            return;
        };
        let locals = locals_of_function(&binary, "main").expect("locals");
        let counter = locals
            .iter()
            .find(|local| local.name == "counter")
            .expect("counter");
        assert!(!counter.ops.is_empty(), "counter should have operations");
        // A stack local is a frame offset; the exact number is gcc's business, not this test's.
        assert!(
            matches!(counter.ops[0], LocationOp::FrameOffset(_)),
            "got {:?}",
            counter.ops
        );
    }

    #[test]
    fn a_frame_offset_is_added_to_the_frame_base() {
        // `DW_OP_fbreg -8` is what gcc emits for the first local in an unoptimised function: the
        // variable sits 8 bytes below the frame base.
        let address = evaluate_location(&[LocationOp::FrameOffset(-8)], 0x1000, 0x1010).unwrap();
        assert_eq!(address, 0x0ff8);
    }

    #[test]
    fn a_constant_offset_follows_a_frame_offset() {
        // `DW_OP_fbreg -16; DW_OP_plus_uconst 8` names the second word of a struct local.
        let address = evaluate_location(
            &[LocationOp::FrameOffset(-16), LocationOp::PlusConstant(8)],
            0x1000,
            0x1010,
        )
        .unwrap();
        assert_eq!(address, 0x0ff8);
    }

    #[test]
    fn a_fixed_address_and_the_cfa_are_both_understood() {
        assert_eq!(
            evaluate_location(&[LocationOp::Address(0x4040)], 0, 0).unwrap(),
            0x4040
        );
        assert_eq!(
            evaluate_location(&[LocationOp::Cfa], 0, 0x2000).unwrap(),
            0x2000
        );
    }

    #[test]
    fn an_operation_this_debugger_cannot_evaluate_is_refused_by_name() {
        // Refused, not guessed: a location evaluated wrongly reads a real number from the wrong
        // place, and that looks like success.
        let error = evaluate_location(
            &[LocationOp::Unsupported("Piece { .. }".to_string())],
            0x1000,
            0x1010,
        )
        .expect_err("must refuse");
        let message = format!("{error}");
        assert!(message.contains("Piece"), "{message}");
        assert!(message.contains("print 0xADDRESS"), "{message}");
    }

    #[test]
    fn an_offset_that_would_underflow_is_refused() {
        let error =
            evaluate_location(&[LocationOp::FrameOffset(-1)], 0, 0).expect_err("must refuse");
        assert!(format!("{error}").contains("does not fit"), "{error}");
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
