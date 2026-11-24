//! NOVA Debugger
//! 
//! Interactive debugger for NOVA programs with support for breakpoints,
//! step execution, variable inspection, and call stack navigation.

// `unsafe` is confined to `target`, where `ptrace` is called, and this attribute is what keeps
// it there: a future `unsafe` block anywhere else fails the build rather than arriving in
// review. See docs/adr/0016-debugger-uses-ptrace.md.
#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod target;
mod commands;
mod session;
mod dwarf;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thiserror::Error;
pub use dwarf::{DwarfParser, DwarfInfo, SymbolTable};

#[derive(Error, Debug)]
pub enum DebuggerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Target process error: {0}")]
    Target(String),
    
    #[error("Breakpoint error: {0}")]
    Breakpoint(String),
    
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    
    #[error("Process not running")]
    ProcessNotRunning,
    
    #[error("Failed to attach to process: {0}")]
    AttachFailed(String),

    #[error("Readline error: {0}")]
    Readline(#[from] rustyline::error::ReadlineError),
}

impl std::fmt::Display for session::StackFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.file, self.line) {
            (Some(file), Some(line)) => {
                write!(f, "{} () at {}:{} (0x{:x})", self.function, file, line, self.address)
            }
            _ => write!(f, "{} () at 0x{:x}", self.function, self.address),
        }
    }
}

#[derive(Parser)]
#[command(name = "nova-dbg")]
#[command(author = "NOVA Team")]
#[command(version = "0.3.0")]
#[command(about = "NOVA Debugger", long_about = None)]
struct Cli {
    /// Binary to debug
    binary: Option<PathBuf>,
    
    /// Process ID to attach to
    #[arg(short, long)]
    pid: Option<u32>,
    
    /// Host and port for remote debugging
    #[arg(short, long)]
    remote: Option<String>,
    
    /// Command script file containing debugger commands, one per line
    #[arg(short = 'c', long = "script")]
    script: Option<PathBuf>,
    
    /// Start in non-interactive mode
    #[arg(short, long)]
    batch: bool,
    
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the program
    Run {
        /// Arguments to pass to the program
        args: Vec<String>,
    },
    
    /// Continue execution until breakpoint
    Continue,
    
    /// Step into function
    Step,
    
    /// Step over function
    Next,
    
    /// Step out of current function
    Finish,
    
    /// Set breakpoint
    Break {
        /// Location (file:line or function name)
        location: String,
        /// Condition expression
        #[arg(short, long)]
        condition: Option<String>,
    },
    
    /// Delete breakpoint
    Delete {
        /// Breakpoint number
        number: u32,
    },
    
    /// Print current location
    Where,
    
    /// Print backtrace
    Backtrace,
    
    /// Print variable value
    Print {
        /// Variable or expression
        expr: String,
    },
    
    /// List source code
    List {
        /// File and line (default: current location)
        location: Option<String>,
    },
    
    /// Attach to running process
    Attach {
        /// Process ID
        pid: u32,
    },
    
    /// Detach from process
    Detach,
    
    /// Kill debugged process
    Kill,
    
    /// Show program status
    Info {
        /// What to show (threads, breakpoints, registers, etc.)
        item: String,
    },
}

fn main() {
    let cli = Cli::parse();
    
    if let Err(e) = run(cli) {
        eprintln!("Debugger error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), DebuggerError> {
    // Determine what to debug
    let target = if let Some(pid) = cli.pid {
        println!("Attaching to process {}...", pid);
        target::attach(pid)?
    } else if let Some(remote) = cli.remote {
        println!("Connecting to remote target {}...", remote);
        target::connect_remote(&remote)?
    } else if let Some(binary) = cli.binary {
        println!("Loading binary {:?}...", binary);
        target::load_binary(&binary)?
    } else {
        return Err(DebuggerError::Target(
            "No target specified. Provide a binary, PID, or use --remote".to_string()
        ));
    };
    
    // Create debug session
    let mut session = session::DebugSession::new(target)?;
    
    // Execute a command script file if provided (one REPL-style command per line)
    if let Some(script) = cli.script {
        return run_script(&mut session, &script);
    }
    
    // Handle subcommands
    if let Some(cmd) = cli.command {
        return execute_command(&mut session, cmd);
    }
    
    // Interactive REPL
    if !cli.batch {
        println!("NOVA Debugger v{}", env!("CARGO_PKG_VERSION"));
        println!("Type 'help' for available commands");
        println!();
        
        repl_loop(&mut session)?;
    }
    
    Ok(())
}

fn execute_command(session: &mut session::DebugSession, cmd: Commands) -> Result<(), DebuggerError> {
    match cmd {
        Commands::Run { args } => session.run(&args),
        Commands::Continue => session.continue_execution(),
        Commands::Step => session.step_into(),
        Commands::Next => session.step_over(),
        Commands::Finish => session.step_out(),
        Commands::Break { location, condition } => {
            let bp = session.set_breakpoint(&location)?;
            if let Some(cond) = condition {
                session.set_breakpoint_condition(bp, &cond)?;
            }
            println!("Breakpoint {} set at {}", bp, location);
            Ok(())
        }
        Commands::Delete { number } => {
            session.delete_breakpoint(number)?;
            println!("Breakpoint {} deleted", number);
            Ok(())
        }
        Commands::Where => {
            let loc = session.current_location()?;
            println!("Current location: {}", loc);
            Ok(())
        }
        Commands::Backtrace => {
            let frames = session.backtrace()?;
            for (i, frame) in frames.iter().enumerate() {
                println!("#{}  {}", i, frame);
            }
            Ok(())
        }
        Commands::Print { expr } => {
            let value = session.evaluate(&expr)?;
            println!("{} = {}", expr, value);
            Ok(())
        }
        Commands::List { location } => {
            let lines = session.list_source(location.as_deref())?;
            for line in lines {
                println!("{}", line);
            }
            Ok(())
        }
        Commands::Attach { pid } => {
            let new_target = target::attach(pid)?;
            session.set_target(new_target)?;
            println!("Attached to process {}", pid);
            Ok(())
        }
        Commands::Detach => {
            session.detach()?;
            println!("Detached from target");
            Ok(())
        }
        Commands::Kill => {
            session.kill()?;
            println!("Process killed");
            Ok(())
        }
        Commands::Info { item } => {
            let info = session.get_info(&item)?;
            println!("{}", info);
            Ok(())
        }
    }
}

/// Execute a script file of debugger commands (one REPL-style command per line).
fn run_script(session: &mut session::DebugSession, path: &std::path::Path) -> Result<(), DebuggerError> {
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match commands::execute_line(session, trimmed) {
            Ok(commands::LineOutcome::Continue) => {}
            Ok(commands::LineOutcome::Quit) => break,
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    Ok(())
}

fn repl_loop(session: &mut session::DebugSession) -> Result<(), DebuggerError> {
    use rustyline::DefaultEditor;
    
    let mut rl = DefaultEditor::new()?;
    
    loop {
        let prompt = match session.state() {
            session::State::Idle => "(nova-dbg) ",
            session::State::Running => "[running] ",
            session::State::Stopped(loc) => &format!("[stopped at {}] ", loc),
            session::State::Exited(code) => &format!("[exited {}] ", code),
        };
        
        match rl.readline(prompt) {
            Ok(line) => {
                let _ = rl.add_history_entry(&line);
                
                if line.trim().is_empty() {
                    continue;
                }

                // One command table, in `commands`, shared with `run_script`. This loop used
                // to carry its own match over the same names, which meant a command added to
                // `parse` was invisible here — `line` was written, compiled, tested and still
                // answered "Unknown command". The duplication was the defect; the arm added to
                // both places was the symptom.
                match commands::execute_line(session, line.trim()) {
                    Ok(commands::LineOutcome::Continue) => {}
                    Ok(commands::LineOutcome::Quit) => break,
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("Interrupted. Type 'quit' to exit.");
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

fn print_help() {
    println!(
        r#"NOVA Debugger Commands:
  
  run [args]       - Start program with optional arguments
  continue (c)     - Continue execution until breakpoint
  step (s)         - Step into function
  next (n)         - Step over function
  finish (f)       - Step out of current function
  
  break (b)        - Set breakpoint (break file:line or break function)
  delete (d)       - Delete breakpoint by number
  
  where (w)        - Show current location
  backtrace (bt)   - Show call stack
  line <address>   - Which source line a hex address is, from the line table
  print (p)        - Print variable/expression value
  list (l)         - List source code
  
  info             - Show info (threads, breakpoints, registers)
  attach           - Attach to running process
  detach           - Detach from process
  kill             - Kill debugged process
  
  quit (q)         - Exit debugger
  help (h)         - Show this help message
"#
    );
}
