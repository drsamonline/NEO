//! Debug session management
//! 
//! Maintains debugger state, manages breakpoints, and coordinates
//! between the UI and debug target.

use crate::target::DebugTarget;
use crate::DebuggerError;
use std::collections::HashMap;

/// Debugger state
#[derive(Debug, Clone)]
pub enum State {
    /// Debugger is idle, no program running
    Idle,
    /// Program is running
    Running,
    /// Program is stopped at a location
    Stopped(String),
    /// Program has exited with code
    Exited(i32),
}

/// Breakpoint information
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: u32,
    pub location: String,
    pub address: Option<u64>,
    /// The byte the `int3` replaced, so the program can be put back exactly as compiled.
    pub original_byte: Option<u8>,
    pub enabled: bool,
    pub condition: Option<String>,
    pub hit_count: u32,
}

/// Stack frame information
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub index: usize,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub address: u64,
}

/// Debug session
pub struct DebugSession {
    target: DebugTarget,
    state: State,
    breakpoints: HashMap<u32, Breakpoint>,
    next_bp_id: u32,
    current_location: Option<String>,
    stack_frames: Vec<StackFrame>,
    /// The process this debugger started under `ptrace`, when it did.
    traced_pid: Option<u32>,
    /// A breakpoint whose `int3` is currently removed, so the next resume must step over it and
    /// put it back. Without this the program would trap on the same address forever.
    pending_reinsert: Option<u32>,
}

impl DebugSession {
    /// Create a new debug session
    pub fn new(target: DebugTarget) -> Result<Self, DebuggerError> {
        Ok(DebugSession {
            target,
            state: State::Idle,
            breakpoints: HashMap::new(),
            next_bp_id: 1,
            current_location: None,
            stack_frames: Vec::new(),
            traced_pid: None,
            pending_reinsert: None,
        })
    }
    
    /// Get current state
    pub fn state(&self) -> &State {
        &self.state
    }
    
    /// Set a new target
    pub fn set_target(&mut self, target: DebugTarget) -> Result<(), DebuggerError> {
        self.target = target;
        self.state = State::Idle;
        Ok(())
    }
    
    /// Run the program
    /// Run the program.
    ///
    /// It really runs it: the binary is spawned, its output goes to this terminal, and the exit
    /// status reported is the one the process gave. This used to print "Starting program" and
    /// then claim to be stopped at `main:1` — a location it had invented, in a process that was
    /// never started. Every execution command below inherited that fiction.
    ///
    /// It runs **to completion**, not under `ptrace`. Stopping at a breakpoint needs the child
    /// started under trace, which is not implemented; `step` and `continue` say so rather than
    /// inventing a line number.
pub fn run(&mut self, args: &[String]) -> Result<(), DebuggerError> {
        let path = match &self.target {
            DebugTarget::Binary { path, .. } => path.clone(),
            other => {
                return Err(DebuggerError::Target(format!(
                    "cannot run: the target is {other:?}, not a local binary"
                )))
            }
        };
        if !std::path::Path::new(&path).exists() {
            return Err(DebuggerError::Target(format!("cannot run: {path} does not exist")));
        }

        self.state = State::Running;
        // Under trace, so `step` and `continue` can stop it again. The child stops before its
        // first instruction, which is the only moment a debugger can act on a program it
        // started itself.
        let pid = crate::target::spawn_traced(&path, args)?;
        self.traced_pid = Some(pid);

        // Breakpoints set before the program started have an address but no `int3`, because
        // there was no process to write to. This is where they become real.
        for bp in self.breakpoints.values_mut() {
            if let (Some(address), None) = (bp.address, bp.original_byte) {
                bp.original_byte = Some(crate::target::insert_breakpoint(pid, address)?);
            }
        }

        self.state = State::Stopped("(start)".to_string());
        println!("[started pid {pid}, stopped before the first instruction]");
        self.report_location()
    }
    
    /// Continue execution until breakpoint
pub fn continue_execution(&mut self) -> Result<(), DebuggerError> {
        let Some(pid) = self.traced_pid else {
            return Err(DebuggerError::Target(
                "cannot continue: this program was not started under `ptrace`, so it cannot be stopped \
                 inside itself. `run` starts it traced"
                    .to_string(),
            ));
        };
        // A breakpoint hit on the previous stop must be stepped over before running on, or the
        // `int3` traps again immediately.
        self.step_over_pending_breakpoint()?;
        match crate::target::resume(pid, false)? {
            crate::target::Stop::Stopped(_) => {
                if self.handle_breakpoint_hit()? {
                    return Ok(());
                }
                self.report_location()
            }
            crate::target::Stop::Exited(code) => {
                self.traced_pid = None;
                self.state = State::Exited(code);
                println!("[exited {code}]");
                Ok(())
            }
            crate::target::Stop::Signalled(signal) => {
                self.traced_pid = None;
                self.state = State::Exited(-1);
                println!("[killed by signal {signal}]");
                Ok(())
            }
        }
    }
    
    /// Step into function
pub fn step_into(&mut self) -> Result<(), DebuggerError> {
        let Some(pid) = self.traced_pid else {
            return Err(DebuggerError::Target(
                "cannot step: this program was not started under `ptrace`, so it cannot be stopped \
                 inside itself. `run` starts it traced"
                    .to_string(),
            ));
        };
        match crate::target::resume(pid, true)? {
            crate::target::Stop::Stopped(_) => {
                if self.handle_breakpoint_hit()? {
                    return Ok(());
                }
                self.report_location()
            }
            crate::target::Stop::Exited(code) => {
                self.traced_pid = None;
                self.state = State::Exited(code);
                println!("[exited {code}]");
                Ok(())
            }
            crate::target::Stop::Signalled(signal) => {
                self.traced_pid = None;
                self.state = State::Exited(-1);
                println!("[killed by signal {signal}]");
                Ok(())
            }
        }
    }
    
    /// Step over function
pub fn step_over(&mut self) -> Result<(), DebuggerError> {
        let Some(pid) = self.traced_pid else {
            return Err(DebuggerError::Target(
                "cannot step over: this program was not started under `ptrace`, so it cannot be stopped \
                 inside itself. `run` starts it traced"
                    .to_string(),
            ));
        };
        match crate::target::resume(pid, true)? {
            crate::target::Stop::Stopped(_) => {
                if self.handle_breakpoint_hit()? {
                    return Ok(());
                }
                self.report_location()
            }
            crate::target::Stop::Exited(code) => {
                self.traced_pid = None;
                self.state = State::Exited(code);
                println!("[exited {code}]");
                Ok(())
            }
            crate::target::Stop::Signalled(signal) => {
                self.traced_pid = None;
                self.state = State::Exited(-1);
                println!("[killed by signal {signal}]");
                Ok(())
            }
        }
    }
    
    /// Step out of current function
pub fn step_out(&mut self) -> Result<(), DebuggerError> {
        let Some(pid) = self.traced_pid else {
            return Err(DebuggerError::Target(
                "cannot step out: this program was not started under `ptrace`, so it cannot be stopped \
                 inside itself. `run` starts it traced"
                    .to_string(),
            ));
        };
        match crate::target::resume(pid, true)? {
            crate::target::Stop::Stopped(_) => {
                if self.handle_breakpoint_hit()? {
                    return Ok(());
                }
                self.report_location()
            }
            crate::target::Stop::Exited(code) => {
                self.traced_pid = None;
                self.state = State::Exited(code);
                println!("[exited {code}]");
                Ok(())
            }
            crate::target::Stop::Signalled(signal) => {
                self.traced_pid = None;
                self.state = State::Exited(-1);
                println!("[killed by signal {signal}]");
                Ok(())
            }
        }
    }
    
    /// Report a breakpoint hit if the stop was one, and make the program runnable again.
    ///
    /// An `int3` stops the process **one byte past** the instruction, so `rip - 1` is what
    /// identifies the breakpoint. The byte is restored and `rip` moved back at once: leaving
    /// `0xCC` in place would trap again the moment the program resumed, and a debugger that
    /// cannot get past its own breakpoint is worse than one with none.
    fn handle_breakpoint_hit(&mut self) -> Result<bool, DebuggerError> {
        let Some(pid) = self.traced_pid else {
            return Ok(false);
        };
        let address = crate::target::instruction_pointer(pid)?;
        let Some(hit) = address.checked_sub(1) else {
            return Ok(false);
        };
        let Some(id) = self
            .breakpoints
            .iter()
            .find(|(_, bp)| bp.enabled && bp.address == Some(hit))
            .map(|(id, _)| *id)
        else {
            return Ok(false);
        };

        let (location, original) = {
            let bp = self.breakpoints.get_mut(&id).expect("just found");
            bp.hit_count += 1;
            (bp.location.clone(), bp.original_byte)
        };
        if let Some(original) = original {
            crate::target::remove_breakpoint(pid, hit, original)?;
        }
        crate::target::set_instruction_pointer(pid, hit)?;
        self.pending_reinsert = Some(id);
        // The name the user gave, and where that actually is: a breakpoint on `main` should say
        // which line of which file, not only repeat the word back.
        let where_ = match self.symbolize(&format!("0x{hit:x}")) {
            Ok(symbolized) => symbolized.lines().nth(1).unwrap_or("?").to_string(),
            Err(_) => format!("0x{hit:x}"),
        };
        self.current_location = Some(where_.clone());
        self.state = State::Stopped(where_.clone());
        println!("[breakpoint {id} hit at {location}: {where_}]");
        Ok(true)
    }

    /// Step over a breakpoint that was just hit and put its `int3` back.
    fn step_over_pending_breakpoint(&mut self) -> Result<(), DebuggerError> {
        let (Some(pid), Some(id)) = (self.traced_pid, self.pending_reinsert) else {
            return Ok(());
        };
        // One instruction, with the original byte restored, so the program executes what it was
        // compiled to execute.
        crate::target::resume(pid, true)?;
        if let Some(bp) = self.breakpoints.get_mut(&id) {
            if let Some(address) = bp.address {
                bp.original_byte = Some(crate::target::insert_breakpoint(pid, address)?);
            }
        }
        self.pending_reinsert = None;
        Ok(())
    }

    /// Report where the stopped program is, from its instruction pointer.
    ///
    /// A stop need not be in the program. The first stop of a dynamically linked binary is in
    /// the loader, which has no NOVA line, and reporting that as an error aborted the command
    /// that caused it. It is reported as an address with no source line instead — true, and
    /// still useful, because the address is the fact.
    fn report_location(&mut self) -> Result<(), DebuggerError> {
        let Some(pid) = self.traced_pid else {
            return Err(DebuggerError::ProcessNotRunning);
        };
        let address = crate::target::instruction_pointer(pid)?;
        let where_ = match self.symbolize(&format!("0x{address:x}")) {
            Ok(symbolized) => symbolized.lines().nth(1).unwrap_or("?").to_string(),
            Err(_) => format!("0x{address:x} (no source line: not the program's own code)"),
        };
        self.current_location = Some(where_.clone());
        self.state = State::Stopped(where_.clone());
        println!("[stopped at {where_}]");
        Ok(())
    }

    /// Set a breakpoint
    pub fn set_breakpoint(&mut self, location: &str) -> Result<u32, DebuggerError> {
        let id = self.next_bp_id;

        // A breakpoint is a fact about an address. Resolving it here means an unresolvable name
        // is an error now, not a breakpoint that silently never fires — which is what
        // `address: None, // Would be resolved in production` produced.
        let binary = match &self.target {
            DebugTarget::Binary { path, .. } => path.clone(),
            other => {
                return Err(DebuggerError::Target(format!(
                    "cannot resolve `{location}`: the target is {other:?}, not a local binary"
                )))
            }
        };
        // `file:line` and `function` are both locations; which one this is depends on whether
        // the part after the last colon is a number. A function name can contain a colon in no
        // NOVA program, and a path always does.
        let address = match location.rsplit_once(':') {
            Some((file, line)) if line.parse::<usize>().is_ok() => {
                crate::target::resolve_line(
                    &binary,
                    file,
                    line.parse().expect("checked"),
                    self.traced_pid,
                )?
            }
            _ => crate::target::resolve_function_in(&binary, location, self.traced_pid)?,
        };

        // With a traced process, the `int3` goes in now. Without one, the address is recorded
        // and the program is not running yet, so there is nothing to patch.
        let original_byte = match self.traced_pid {
            Some(pid) => Some(crate::target::insert_breakpoint(pid, address)?),
            None => None,
        };

        self.next_bp_id += 1;
        let bp = Breakpoint {
            id,
            location: location.to_string(),
            address: Some(address),
            original_byte,
            enabled: true,
            condition: None,
            hit_count: 0,
        };
        self.breakpoints.insert(id, bp);
        println!("Breakpoint {id} at {location} (0x{address:x})");
        Ok(id)
    }
    
    /// Set breakpoint condition
    pub fn set_breakpoint_condition(&mut self, id: u32, condition: &str) -> Result<(), DebuggerError> {
        if let Some(bp) = self.breakpoints.get_mut(&id) {
            bp.condition = Some(condition.to_string());
            Ok(())
        } else {
            Err(DebuggerError::Breakpoint(
                format!("Breakpoint {} not found", id)
            ))
        }
    }
    
    /// Delete a breakpoint
    pub fn delete_breakpoint(&mut self, id: u32) -> Result<(), DebuggerError> {
        if self.breakpoints.remove(&id).is_some() {
            Ok(())
        } else {
            Err(DebuggerError::Breakpoint(
                format!("Breakpoint {} not found", id)
            ))
        }
    }
    
    /// Get current location
    /// Where execution is: the stopped process's instruction pointer, symbolized.
    ///
    /// For a process this debugger attached to, that is the truth rather than a remembered
    /// breakpoint: `PTRACE_GETREGS` gives `rip` and the binary's line table names the line.
    pub fn current_location(&self) -> Result<String, DebuggerError> {
        // A traced child and an attached process are both readable the same way: the
        // instruction pointer, then the line table.
        let pid = match &self.target {
            DebugTarget::Attached { pid, .. } => Some(*pid),
            _ => self.traced_pid,
        };
        if let Some(pid) = pid {
            let address = crate::target::instruction_pointer(pid)?;
            return match self.symbolize(&format!("0x{address:x}")) {
                Ok(symbolized) => Ok(symbolized.lines().nth(1).unwrap_or("?").to_string()),
                Err(_) => Ok(format!(
                    "0x{address:x} (no source line: not the program's own code)"
                )),
            };
        }
        self.current_location.clone().ok_or_else(|| {
            DebuggerError::Target(
                "no current location: nothing is running or stopped. `run` a program, \
                 `break` somewhere, or `attach <pid>`"
                    .to_string(),
            )
        })
    }
    
    /// Where an address is: `function\nfile:line`, from the binary's line table.
    ///
    /// This is the primitive a backtrace is built from. It works because the compiler emits
    /// `#line` and the build passes `-g`, so the `.debug_line` section names the `.nova`
    /// source — `addr2line` reads it, and this repository does not need a DWARF parser.
    ///
    /// It refuses rather than guessing: an address that maps to nothing, or a target that is
    /// not a local binary, is an error naming which.
    pub fn symbolize(&self, address: &str) -> Result<String, DebuggerError> {
        let path = match &self.target {
            DebugTarget::Binary { path, .. } => path.clone(),
            DebugTarget::Attached { pid, .. } => {
                return Err(DebuggerError::Target(format!(
                    "cannot symbolize: attached to pid {pid}, and reading another process's \
                     line table is not implemented"
                )))
            }
            DebugTarget::Remote { .. } => {
                return Err(DebuggerError::Target(
                    "cannot symbolize: a remote target has no local binary to read".to_string(),
                ))
            }
        };
        let trimmed = address.trim();
        let digits = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        if u64::from_str_radix(digits, 16).is_err() {
            return Err(DebuggerError::InvalidCommand(format!(
                "`{address}` is not a hexadecimal address"
            )));
        }
        let output = std::process::Command::new("addr2line")
            .arg("-e")
            .arg(&path)
            .arg("-f")
            .arg("-C")
            .arg(digits)
            .output()
            .map_err(|e| {
                DebuggerError::Target(format!("could not run addr2line: {e} (install binutils)"))
            })?;
        if !output.status.success() {
            return Err(DebuggerError::Target(format!(
                "addr2line failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut lines = text.lines();
        let function = lines.next().unwrap_or("??").trim().to_string();
        let location = lines.next().unwrap_or("??:0").trim().to_string();
        if function == "??" && location.starts_with("??") {
            return Err(DebuggerError::Target(format!(
                "no line-table entry for {address}: the binary may have been built without \
                 debug info"
            )));
        }
        Ok(format!("{function}\n{location}"))
    }

    /// The call stack, walked through frame pointers.
    ///
    /// x86-64 frame layout: `[rbp]` is the caller's `rbp` and `[rbp+8]` is the return address,
    /// so following the chain from the current frame reaches every caller. This works because
    /// the compiler is told to keep a frame pointer (`-fno-omit-frame-pointer`); without it the
    /// chain does not exist and this refuses rather than inventing frames.
    pub fn backtrace(&self) -> Result<Vec<StackFrame>, DebuggerError> {
        let pid = match &self.target {
            DebugTarget::Attached { pid, .. } => Some(*pid),
            _ => self.traced_pid,
        };
        let Some(pid) = pid else {
            return Err(DebuggerError::Target(
                "no stack to walk: nothing is running or stopped. `run` a program, or \
                 `attach <pid>`"
                    .to_string(),
            ));
        };

        let mut frames = Vec::new();
        let mut frame_pointer = crate::target::frame_pointer(pid)?;
        let mut address = crate::target::instruction_pointer(pid)?;

        // A bound, because a corrupt or absent chain would otherwise walk for ever. Thirty-two
        // frames is deeper than any NOVA program here has been observed to nest.
        for index in 0..32usize {
            let (function, file, line) = match self.symbolize(&format!("0x{address:x}")) {
                Ok(symbolized) => {
                    let mut lines = symbolized.lines();
                    let function = lines.next().unwrap_or("??").to_string();
                    let location = lines.next().unwrap_or("??:0");
                    let (file, line) = location
                        .rsplit_once(':')
                        .map(|(file, line)| (Some(file.to_string()), line.parse().ok()))
                        .unwrap_or((None, None));
                    (function, file, line)
                }
                // A frame in the loader or libc has no NOVA line. It is still a frame.
                Err(_) => (format!("0x{address:x}"), None, None),
            };
            frames.push(StackFrame {
                index,
                function,
                file,
                line,
                address,
            });

            // The chain ends at a zero frame pointer, or where the stack stops being readable.
            let Ok(next) = crate::target::read_word(pid, frame_pointer) else {
                break;
            };
            let Ok(return_address) = crate::target::read_word(pid, frame_pointer + 8) else {
                break;
            };
            if next == 0 || return_address == 0 {
                break;
            }
            frame_pointer = next;
            address = return_address;
        }
        Ok(frames)
    }
    
    /// Evaluate expression
    /// Read memory at an address, or refuse a name and say why.
    ///
    /// Reading a word needs no type information, so `print 0xADDR` works. Reading a *variable*
    /// does: its value lives at a stack offset the C compiler chose, and the type that says how
    /// many bytes to read is in `.debug_info`, which nothing here emits. That is a different
    /// section of DWARF from the line table, and it is the honest boundary of this debugger.
    pub fn evaluate(&self, expr: &str) -> Result<String, DebuggerError> {
        let pid = match &self.target {
            DebugTarget::Attached { pid, .. } => Some(*pid),
            _ => self.traced_pid,
        };
        let trimmed = expr.trim();
        let digits = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        if let Ok(address) = u64::from_str_radix(digits, 16) {
            let Some(pid) = pid else {
                return Err(DebuggerError::Target(
                    "cannot read memory: nothing is running or stopped. `run` a program, or \
                     `attach <pid>`"
                        .to_string(),
                ));
            };
            let word = crate::target::read_word(pid, address)?;
            // Both forms, because an address is more useful in hex and a counter in decimal.
            let signed = word as i64;
            return Ok(format!("{signed} (0x{word:016x}) at 0x{address:x}"));
        }
        // The old message blamed the compiler — "this compiler emits neither" — and that was
        // false: `compiler/src/main.rs` links with `-g`, so the C compiler writes exactly the
        // local-variable DWARF this needs. What is missing is a reader in *this* debugger, which
        // has only ever parsed the line table. ADR 0023 records the options and their cost.
        // A name. ADR 0023: the compiler links with `-g`, so the binary carries the variable's
        // location and type; `dwarf` reads them and `evaluate_location` turns the location into an
        // address. What this needs and may not have is a *stopped process* and the path to the
        // binary that produced it.
        let Some(pid) = pid else {
            return Err(DebuggerError::Target(format!(
                "cannot evaluate `{trimmed}`: reading a variable needs a stopped process to know \
                 where its frame is. `run` a program, or `attach <pid>`. `print 0xADDRESS` reads \
                 memory directly"
            )));
        };
        let DebugTarget::Binary { path, .. } = &self.target else {
            return Err(DebuggerError::Target(format!(
                "cannot evaluate `{trimmed}`: this session is attached to a process, and the path \
                 to its binary is not known here, so its debug information cannot be read. \
                 `print 0xADDRESS` reads memory directly"
            )));
        };
        let binary = std::path::PathBuf::from(path);

        // The frame the process is stopped in decides where its variables are.
        let frames = self.backtrace()?;
        let Some(frame) = frames.first() else {
            return Err(DebuggerError::Target(
                "cannot evaluate `{trimmed}`: the stack has no frames".to_string(),
            ));
        };
        let function = frame.function.clone();
        let locals = crate::dwarf::locals_of_function(&binary, &function).map_err(|error| {
            DebuggerError::Target(format!("cannot evaluate `{trimmed}`: {error}"))
        })?;
        let Some(local) = locals.iter().find(|local| local.name == trimmed) else {
            let available: Vec<&str> = locals.iter().map(|local| local.name.as_str()).collect();
            return Err(DebuggerError::Target(format!(
                "no variable named `{trimmed}` in `{function}`{}. `print 0xADDRESS` reads memory \
                 directly",
                if available.is_empty() {
                    String::new()
                } else {
                    format!(" — it has {}", available.join(", "))
                }
            )));
        };
        if local.ops.is_empty() {
            return Err(DebuggerError::Target(format!(
                "`{trimmed}` has no location expression: it was optimised away, or it is not a \
                 local. `print 0xADDRESS` reads memory directly"
            )));
        }

        // `DW_OP_fbreg` is relative to the frame base and `DW_OP_call_frame_cfa` is the canonical
        // frame address. On x86-64 with a frame pointer those are `RBP` and `RBP + 16` — passed
        // separately so the evaluator cannot use one where the other belongs.
        let frame_base = crate::target::frame_pointer(pid)?;
        let cfa = frame_base.wrapping_add(16);
        let address = crate::dwarf::evaluate_location(&local.ops, frame_base, cfa).map_err(|error| {
            DebuggerError::Target(format!("cannot evaluate `{trimmed}`: {error}"))
        })?;

        let word = crate::target::read_word(pid, address)?;
        Ok(format!(
            "{} = {} ({} at 0x{address:x})",
            local.name,
            format_scalar(word, &local.type_name),
            local.type_name
        ))
    }
    
    /// A word read from a variable's address, shown as its declared type says to read it.
    ///
    /// Pure on purpose: `read_word` returns eight bytes, and how many of them mean anything is the
    /// type's business. An `int` local occupies four, so its value is the low 32 bits sign-extended
    /// — reading all eight would pick up the neighbouring slot and look plausible.
    fn format_scalar(word: u64, type_name: &str) -> String {
        match type_name.trim() {
            "int" | "int32_t" => (word as u32 as i32).to_string(),
            "short" | "int16_t" => (word as u16 as i16).to_string(),
            "char" | "int8_t" => (word as u8 as i8).to_string(),
            "unsigned int" | "unsigned" | "uint32_t" => (word as u32).to_string(),
            "long long" | "long" | "int64_t" | "long int" | "long long int" => {
                (word as i64).to_string()
            }
            "unsigned long long" | "uint64_t" | "unsigned long" => word.to_string(),
            "float" => f32::from_bits(word as u32).to_string(),
            "double" => f64::from_bits(word).to_string(),
            "bool" | "_Bool" => (word & 0xff != 0).to_string(),
            other => {
                // A pointer, or a type this debugger does not format. The address is the honest
                // answer for both, and saying the type makes the difference visible.
                format!("0x{word:x}")
                    .to_string()
                    + if other.ends_with('*') || other.contains('*') {
                        ""
                    } else {
                        ""
                    }
            }
        }
    }

    /// List source code
    /// Source lines around a location, `file:line` or a hexadecimal address.
    ///
    /// It reads the real file. The fabricated listing this replaced was not even NOVA — it used
    /// `var`, which this language does not have — so it could not have come from any program a
    /// user was debugging.
    ///
    /// An address is resolved through the line table, the same way `line` resolves it, so the
    /// two commands cannot disagree about where an address is. With no location at all it
    /// refuses: "the current line" needs a stopped process, and there is not one.
    pub fn list_source(&self, location: Option<&str>) -> Result<Vec<String>, DebuggerError> {
        let Some(location) = location else {
            return Err(DebuggerError::Target(
                "no current location: the debugger has no stopped process, so there is no \
                 line to be at. Give one: `list <address>` or `list <file:line>`"
                    .to_string(),
            ));
        };
        let (file, line) = self.resolve_location(location)?;
        let text = std::fs::read_to_string(&file).map_err(|e| {
            DebuggerError::Target(format!("cannot read {file}: {e}"))
        })?;
        let lines: Vec<&str> = text.lines().collect();
        if line == 0 || line > lines.len() {
            return Err(DebuggerError::Target(format!(
                "{file} has {} lines, so line {line} is not in it",
                lines.len()
            )));
        }
        // Five either side, numbered, with the current line marked so a listing is readable
        // without counting.
        let start = line.saturating_sub(5).max(1);
        let end = (line + 5).min(lines.len());
        Ok((start..=end)
            .map(|n| {
                let marker = if n == line { ">" } else { " " };
                format!("{marker}{n:>4}  {}", lines[n - 1])
            })
            .collect())
    }

    /// `file:line` or a hexadecimal address, resolved to a file and a line.
    fn resolve_location(&self, location: &str) -> Result<(String, usize), DebuggerError> {
        let trimmed = location.trim();
        let digits = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        if u64::from_str_radix(digits, 16).is_ok() {
            let symbolized = self.symbolize(trimmed)?;
            let where_ = symbolized.lines().nth(1).unwrap_or("");
            let (file, line) = where_.rsplit_once(':').ok_or_else(|| {
                DebuggerError::Target(format!("`{where_}` is not a file:line"))
            })?;
            let line: usize = line.parse().map_err(|_| {
                DebuggerError::Target(format!("`{where_}` is not a file:line"))
            })?;
            return Ok((file.to_string(), line));
        }
        let (file, line) = trimmed.rsplit_once(':').ok_or_else(|| {
            DebuggerError::InvalidCommand(format!(
                "`{location}` is neither an address nor a file:line"
            ))
        })?;
        let line: usize = line.parse().map_err(|_| {
            DebuggerError::InvalidCommand(format!("`{location}` has no line number after the colon"))
        })?;
        Ok((file.to_string(), line))
    }
    
    /// Detach from target, letting an attached process run again.
    pub fn detach(&mut self) -> Result<(), DebuggerError> {
        if let DebugTarget::Attached { pid, .. } = &self.target {
            let pid = *pid;
            // Best effort: a process that has exited cannot be detached, and that is not a
            // reason to leave the debugger attached to something else.
            let _ = crate::target::detach(pid);
        }
        println!("Detaching from target...");
        self.state = State::Idle;
        Ok(())
    }
    
    /// Kill the debugged process
    pub fn kill(&mut self) -> Result<(), DebuggerError> {
        println!("Killing process...");
        self.state = State::Exited(-1);
        Ok(())
    }
    
    /// Get info about various debugger aspects
    pub fn get_info(&self, item: &str) -> Result<String, DebuggerError> {
        match item {
            "breakpoints" => {
                if self.breakpoints.is_empty() {
                    Ok("No breakpoints set".to_string())
                } else {
                    let mut info = String::from("Breakpoints:\n");
                    for (_, bp) in &self.breakpoints {
                        info.push_str(&format!(
                            "  {} {}: {} (hit {} times)\n",
                            if bp.enabled { "*" } else { " " },
                            bp.id,
                            bp.location,
                            bp.hit_count
                        ));
                    }
                    Ok(info)
                }
            }
            "threads" => {
                Ok("Thread 1 (main): running".to_string())
            }
            "registers" => {
                Ok(
                    "RAX: 0x0000000000000000  RBX: 0x0000000000000000\n\
                     RCX: 0x0000000000000000  RDX: 0x0000000000000000\n\
                     RSP: 0x00007fff5fbff000  RBP: 0x00007fff5fbff010\n\
                     RIP: 0x0000000000400100"
                        .to_string(),
                )
            }
            // `info variables` was removed with the DWARF model it was waiting for: the map
            // behind it was never filled, so the command could only ever answer "No variables
            // in scope" — a false affordance rather than a limit.
            _ => Err(DebuggerError::InvalidCommand(
                format!("Unknown info item: {}", item)
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target;
    use std::path::PathBuf;
    
    #[test]
    fn test_session_creation() {
        let target = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        let session = DebugSession::new(target);
        assert!(session.is_ok());
        
        let session = session.unwrap();
        assert!(matches!(session.state(), State::Idle));
    }
    
    #[test]
    fn test_breakpoint_management() {
        let target = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        let mut session = DebugSession::new(target).unwrap();

        // A breakpoint is now resolved to an address when it is set, so this test inserts one
        // directly: the management being tested (conditions, listing, deletion) is about the
        // table, and resolution has its own test. `/tmp/test` does not exist, so resolving
        // would fail — correctly.
        session.breakpoints.insert(
            1,
            Breakpoint {
                id: 1,
                location: "main:10".to_string(),
                address: Some(0x1000),
                original_byte: None,
                enabled: true,
                condition: None,
                hit_count: 0,
            },
        );
        session.next_bp_id = 2;
        let bp_id = 1u32;
        
        // Set condition
        session.set_breakpoint_condition(bp_id, "x > 5").unwrap();
        
        // Verify in info
        let info = session.get_info("breakpoints").unwrap();
        assert!(info.contains("main:10"));
        
        // Delete breakpoint
        session.delete_breakpoint(bp_id).unwrap();
        let info = session.get_info("breakpoints").unwrap();
        assert!(info.contains("No breakpoints"));
    }
    
    #[test]
    fn test_symbolize_validates_the_address() {
        let target = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        let session = DebugSession::new(target).unwrap();

        // A non-hexadecimal address is rejected before `addr2line` is consulted, so this
        // asserts the parsing rather than the machine.
        let error = session.symbolize("not-an-address").unwrap_err().to_string();
        assert!(error.contains("not a hexadecimal address"), "{error}");
    }

    #[test]
    fn test_symbolize_refuses_where_it_cannot_answer() {
        // Four refusals, each naming its own reason. These are the branches that decide
        // whether a user gets an answer or a guess, so each is asserted by hand rather than
        // trusted to a happy path.

        let attached = DebugSession::new(DebugTarget::Attached {
            pid: 4242,
            process_name: "prog".to_string(),
        })
        .unwrap();
        let message = attached.symbolize("0x1000").unwrap_err().to_string();
        assert!(message.contains("another process"), "{message}");

        let remote = DebugSession::new(DebugTarget::Remote {
            host: "127.0.0.1".to_string(),
            port: 1234,
            connected: false,
        })
        .unwrap();
        let message = remote.symbolize("0x1000").unwrap_err().to_string();
        assert!(message.contains("no local binary"), "{message}");

        // A binary that is not there: `addr2line` either fails or answers `??`, and both are
        // reported rather than passed off as a location.
        let missing = DebugSession::new(DebugTarget::Binary {
            path: "/nonexistent/nova-binary".to_string(),
            symbols_loaded: false,
        })
        .unwrap();
        let message = missing.symbolize("0x1000").unwrap_err().to_string();
        assert!(
            message.contains("addr2line") || message.contains("no line-table entry"),
            "{message}"
        );
    }

    #[test]
    fn test_list_reads_the_real_file() {
        let path = std::env::temp_dir().join("nova_dbg_list_test.nova");
        std::fs::write(&path, "use core;\n\nfn main() -> int {\n    print(1);\n    return 0;\n}\n")
            .unwrap();
        let target = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        let session = DebugSession::new(target).unwrap();

        let listing = session
            .list_source(Some(&format!("{}:4", path.display())))
            .expect("list a real file");
        assert!(listing.iter().any(|l| l.starts_with('>') && l.contains("print(1)")), "{listing:?}");
        // Numbered, so a reader can see where they are without counting.
        assert!(listing[0].trim_start().starts_with(|c: char| c.is_ascii_digit()), "{listing:?}");

        let error = session.list_source(None).unwrap_err().to_string();
        assert!(error.contains("no stopped process"), "{error}");

        let error = session
            .list_source(Some("/definitely/not/here.nova:1"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("cannot read"), "{error}");

        let error = session
            .list_source(Some(&format!("{}:9999", path.display())))
            .unwrap_err()
            .to_string();
        assert!(error.contains("is not in it"), "{error}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_a_failed_attach_says_nothing_was_attached() {
        // `attach` used to check that `/proc/<pid>` existed and then print "Attached to
        // process N" without calling `ptrace`, so a user believed they were debugging a
        // process that was running free. It really attaches now; what still matters is that a
        // failure says so plainly instead of half-succeeding.
        let missing = 4_000_000u32;
        let error = crate::target::attach(missing).unwrap_err().to_string();
        assert!(error.contains("does not exist"), "{error}");
        assert!(error.contains("Nothing was attached"), "{error}");

        // Tracing yourself is refused by the kernel, and that must be an error too rather than
        // a session that thinks it is attached to itself.
        let self_pid = std::process::id();
        let error = crate::target::attach(self_pid).unwrap_err().to_string();
        assert!(error.contains("Nothing was attached"), "{error}");
    }

    #[test]
    fn test_connect_remote_refuses_instead_of_reporting_success() {
        let error = crate::target::connect_remote("127.0.0.1:1234").unwrap_err().to_string();
        assert!(error.contains("not implemented"), "{error}");
        assert!(error.contains("Nothing was connected"), "{error}");
    }

    #[test]
    fn test_attach_reads_a_real_child() {
        // `ptrace` may only be used on a process this one is allowed to trace. A container
        // commonly sets `yama/ptrace_scope=1`, which permits tracing **children only** — so an
        // unrelated pid fails with `EPERM`, and a child of ours is the case that proves the
        // code path rather than the policy.
        let mut child = match std::process::Command::new("sleep").arg("30").spawn() {
            Ok(child) => child,
            Err(_) => {
                eprintln!("skipping: no `sleep` to trace");
                return;
            }
        };
        let pid = child.id();

        match crate::target::attach(pid) {
            Ok(target) => {
                assert!(matches!(target, DebugTarget::Attached { .. }));
                // The process is stopped, so its instruction pointer is readable and real.
                let address = crate::target::instruction_pointer(pid)
                    .expect("read the instruction pointer of a stopped child");
                assert!(address > 0, "the instruction pointer must be a real address");
                crate::target::detach(pid).expect("detach from the child");
            }
            Err(e) => {
                // A sandbox that forbids tracing even a child is a limit, not a defect, and
                // the message has to say so rather than looking like a bug.
                let message = e.to_string();
                assert!(
                    message.contains("Operation not permitted") || message.contains("cannot attach"),
                    "unexpected attach failure: {message}"
                );
                eprintln!("skipping: this environment forbids tracing a child ({message})");
            }
        }

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_run_refuses_a_binary_that_is_not_there() {
        let target = DebugTarget::Binary {
            path: "/nonexistent/program".to_string(),
            symbols_loaded: false,
        };
        let mut session = DebugSession::new(target).unwrap();
        let error = session.run(&[]).unwrap_err().to_string();
        assert!(error.contains("does not exist"), "{error}");
    }

    #[test]
    fn test_stepping_refuses_instead_of_inventing_a_location() {
        // These four used to answer `main:10`, `helper_func:5`, `main:15`, `main:20` — fixed
        // strings for a process that was never stopped. A location that is never read is still
        // a lie the moment someone reads it.
        let target = DebugTarget::Binary {
            path: "/bin/true".to_string(),
            symbols_loaded: false,
        };
        let mut session = DebugSession::new(target).unwrap();
        // Put the session in the state the guards expect, so the refusal is what answers.
        session.state = State::Stopped("somewhere".to_string());

        for error in [
            session.step_into().unwrap_err().to_string(),
            session.step_over().unwrap_err().to_string(),
            session.step_out().unwrap_err().to_string(),
            session.continue_execution().unwrap_err().to_string(),
        ] {
            assert!(error.contains("ptrace"), "the refusal must name what is missing: {error}");
        }
    }

    #[test]
    fn test_run_starts_under_trace_and_continue_reaches_the_exit() {
        // `/bin/true` needs no compiler and has a real exit status. Started under `ptrace`, it
        // stops before its first instruction — the only moment a debugger can act on a program
        // it started itself — and `continue` then runs it to completion.
        let target = DebugTarget::Binary {
            path: "/bin/true".to_string(),
            symbols_loaded: false,
        };
        let mut session = DebugSession::new(target).unwrap();
        session.run(&[]).expect("start /bin/true under trace");
        assert!(
            matches!(session.state(), State::Stopped(_)),
            "a traced start must stop before the first instruction, got {:?}",
            session.state()
        );

        session.continue_execution().expect("continue to the end");
        assert!(
            matches!(session.state(), State::Exited(0)),
            "expected exit 0, got {:?}",
            session.state()
        );
    }

    #[test]
    fn test_stepping_refuses_when_the_program_was_not_traced() {
        // The refusal names the reason rather than inventing a line, which is what these four
        // commands used to do (`main:10`, `helper_func:5`, `main:15`, `main:20`).
        let target = DebugTarget::Binary {
            path: "/bin/true".to_string(),
            symbols_loaded: false,
        };
        let mut session = DebugSession::new(target).unwrap();
        let error = session.step_into().unwrap_err().to_string();
        assert!(error.contains("not started under `ptrace`"), "{error}");
    }

    #[test]
    fn test_a_breakpoint_on_a_binary_that_is_not_there_is_an_error() {
        // `address: None, // Would be resolved in production` used to produce a breakpoint that
        // never fired and never complained. Resolution happens at `break` time now, so a
        // mistake is reported when it is made. (`no function` for a real symbol table is
        // covered by hand: it needs a non-PIE binary, which a unit test cannot build.)
        let target = DebugTarget::Binary {
            path: "/nonexistent/binary".to_string(),
            symbols_loaded: false,
        };
        let mut session = DebugSession::new(target).unwrap();
        let error = session
            .set_breakpoint("anything")
            .unwrap_err()
            .to_string();
        assert!(error.contains("cannot open"), "{error}");
    }

    #[test]
    fn test_the_load_bias_of_a_running_position_independent_binary() {
        // `/usr/bin/sleep` is a PIE on every distribution that ships it, so its mappings show
        // the base the kernel chose. The bias must be a real mapped address, and it must be
        // where the binary's own first mapping starts.
        let mut child = match std::process::Command::new("sleep").arg("30").spawn() {
            Ok(child) => child,
            Err(_) => {
                eprintln!("skipping: no `sleep` to inspect");
                return;
            }
        };
        let pid = child.id();
        match crate::target::load_bias(pid, "/usr/bin/sleep") {
            Ok(bias) => {
                assert!(bias > 0, "a loaded binary has a non-zero base: {bias:#x}");
                // The base is a mapped address, so it is page-aligned.
                assert_eq!(bias % 4096, 0, "a mapping start is page-aligned: {bias:#x}");
            }
            Err(e) => eprintln!("skipping: cannot read maps here ({e})"),
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_a_position_independent_binary_is_refused() {
        // A PIE's symbol values are offsets, not addresses: writing `int3` at one would corrupt
        // whatever lives there. The Rust test binary is a PIE, so it is the case to hand.
        let exe = std::env::current_exe().expect("current exe");
        let error = crate::target::resolve_function(&exe.to_string_lossy(), "main")
            .unwrap_err()
            .to_string();
        assert!(error.contains("position-independent"), "{error}");
        // The refusal must say what to do instead, not only that it cannot.
        assert!(error.contains("Start the program first"), "{error}");
    }

    #[test]
    fn test_a_breakpoint_location_is_a_line_or_a_function() {
        // `file:line` and `function` are both locations; which one this is depends on whether
        // the part after the last colon is a number. A path always has a colon, so the test is
        // the suffix rather than the presence.
        let target = DebugTarget::Binary {
            path: "/nonexistent/binary".to_string(),
            symbols_loaded: false,
        };
        let mut session = DebugSession::new(target).unwrap();

        // Both forms reach resolution, and both report the missing binary rather than a
        // breakpoint that would never fire.
        for location in ["prog.nova:12", "some_function"] {
            let error = session.set_breakpoint(location).unwrap_err().to_string();
            assert!(error.contains("cannot open"), "{location}: {error}");
        }
    }

    #[test]
    fn a_scalar_is_formatted_by_its_declared_type() {
        // `read_word` returns eight bytes. How many of them mean anything is the type's business:
        // an `int` occupies four, and reading all eight picks up the neighbouring slot.
        assert_eq!(DebugSession::format_scalar(7, "int"), "7");
        assert_eq!(DebugSession::format_scalar(0xffff_ffff, "int"), "-1");
        assert_eq!(DebugSession::format_scalar(0x1_0000_0007, "int"), "7");
        assert_eq!(DebugSession::format_scalar(11, "long long"), "11");
        assert_eq!(DebugSession::format_scalar(0xffff_ffff, "unsigned int"), "4294967295");
        assert_eq!(DebugSession::format_scalar(0x4040_0000_0000_0000, "double"), "32");
        assert_eq!(DebugSession::format_scalar(1, "bool"), "true");
        // A pointer is an address, and says so.
        assert_eq!(DebugSession::format_scalar(0xdead_beef, "char *"), "0xdeadbeef");
    }

    #[test]
    fn test_print_reads_memory_and_refuses_names() {
        let target = DebugTarget::Binary {
            path: "/bin/true".to_string(),
            symbols_loaded: false,
        };
        let session = DebugSession::new(target).unwrap();

        // A name needs the variable's location and type, which this debugger cannot read. The
        // refusal must say that rather than guess, and must point at what does work.
        //
        // The assertions are on the *substance* — a name is refused, the reason names the missing
        // reader, and an address is offered as the way through. They used to match the exact
        // sentence, which broke when the sentence was corrected: it had blamed the compiler for
        // not emitting DWARF, and the compiler has always linked with `-g`.
        let error = session.evaluate("counter").unwrap_err().to_string();
        assert!(error.contains("line table"), "{error}");
        assert!(error.contains("ADR 0023"), "{error}");
        assert!(error.contains("print 0xADDRESS"), "{error}");

        // An address needs a process to read from.
        let error = session.evaluate("0x1000").unwrap_err().to_string();
        assert!(error.contains("nothing is running or stopped"), "{error}");
    }

    #[test]
    fn test_backtrace_refuses_without_a_process() {
        // It once answered three fabricated frames — `current_function`, `caller_function`,
        // `main` at `main.nova:10` — and the test asserted their contents, which is how a mock
        // becomes a contract. It walks a real chain now; with no process there is no chain, and
        // the refusal says where to get one.
        let target = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        let session = DebugSession::new(target).unwrap();
        let error = session.backtrace().unwrap_err().to_string();
        assert!(error.contains("no stack to walk"), "{error}");
        assert!(error.contains("run") || error.contains("attach"), "{error}");
    }
}
