//! Debugger command parsing and execution
//!
//! Provides a small parser for REPL-style debugger command lines
//! (`break`, `run`, `backtrace`, ...) and an executor that maps parsed
//! commands onto [`DebugSession`] operations. This module is shared by the
//! interactive REPL and the `--script` batch mode.

use crate::session::DebugSession;
use crate::target;
use crate::DebuggerError;

/// A parsed debugger command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `run [args...]` - start the program with optional arguments
    Run { args: Vec<String> },
    /// `continue` / `c` - resume execution until breakpoint
    Continue,
    /// `step` / `s` - step into
    Step,
    /// `next` / `n` - step over
    Next,
    /// `finish` / `f` - step out of current function
    Finish,
    /// `break <location> [condition]` / `b`
    Break { location: String, condition: Option<String> },
    /// `delete <number>` / `d`
    Delete { number: u32 },
    /// `where` / `w` - show current location
    Where,
    /// `backtrace` / `bt` - show call stack
    Backtrace,
    /// `print <expr>` / `p <expr>`
    Print { expr: String },
    /// `list [location]` / `l`
    List { location: Option<String> },
    /// `info <item>` / `i <item>`
    Info { item: String },
    /// `attach <pid>`
    Attach { pid: u32 },
    /// `detach`
    Detach,
    /// `kill`
    Kill,
    /// `help` / `h` / `?`
    Help,
    /// `quit` / `q` / `exit`
    Quit,
}

/// Outcome of executing a command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineOutcome {
    /// Keep the session alive and read the next line.
    Continue,
    /// The user asked to quit; stop reading.
    Quit,
}

/// Parse a single command line into a [`Command`].
///
/// Returns `Err(DebuggerError::InvalidCommand)` for empty or unknown input.
pub fn parse(line: &str) -> Result<Command, DebuggerError> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    let (&name, rest) = parts.split_first().ok_or_else(|| {
        DebuggerError::InvalidCommand("empty command".to_string())
    })?;

    let cmd = match name {
        "run" | "r" => Command::Run { args: rest.iter().map(|s| s.to_string()).collect() },
        "continue" | "c" => Command::Continue,
        "step" | "s" => Command::Step,
        "next" | "n" => Command::Next,
        "finish" | "f" => Command::Finish,
        "break" | "b" => {
            let location = rest
                .first()
                .ok_or_else(|| DebuggerError::InvalidCommand(
                    "usage: break <file:line|function> [condition]".to_string(),
                ))?
                .to_string();
            let condition = if rest.len() > 1 { Some(rest[1..].join(" ")) } else { None };
            Command::Break { location, condition }
        }
        "delete" | "d" => {
            let number = rest
                .first()
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| DebuggerError::InvalidCommand(
                    "usage: delete <breakpoint-number>".to_string(),
                ))?;
            Command::Delete { number }
        }
        "where" | "w" => Command::Where,
        "backtrace" | "bt" => Command::Backtrace,
        "print" | "p" => {
            if rest.is_empty() {
                return Err(DebuggerError::InvalidCommand(
                    "usage: print <expression>".to_string(),
                ));
            }
            Command::Print { expr: rest.join(" ") }
        }
        "list" | "l" => Command::List { location: rest.first().map(|s| s.to_string()) },
        "info" | "i" => {
            let item = rest
                .first()
                .ok_or_else(|| DebuggerError::InvalidCommand(
                    "usage: info <threads|breakpoints|registers|variables>".to_string(),
                ))?
                .to_string();
            Command::Info { item }
        }
        "attach" => {
            let pid = rest
                .first()
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| DebuggerError::InvalidCommand(
                    "usage: attach <pid>".to_string(),
                ))?;
            Command::Attach { pid }
        }
        "detach" => Command::Detach,
        "kill" => Command::Kill,
        "help" | "h" | "?" => Command::Help,
        "quit" | "q" | "exit" => Command::Quit,
        other => return Err(DebuggerError::InvalidCommand(format!(
            "unknown command '{}' (try 'help')", other
        ))),
    };
    Ok(cmd)
}

/// Parse and execute one command line against the given session.
pub fn execute_line(session: &mut DebugSession, line: &str) -> Result<LineOutcome, DebuggerError> {
    match parse(line)? {
        Command::Run { args } => session.run(&args)?,
        Command::Continue => session.continue_execution()?,
        Command::Step => session.step_into()?,
        Command::Next => session.step_over()?,
        Command::Finish => session.step_out()?,
        Command::Break { location, condition } => {
            let bp = session.set_breakpoint(&location)?;
            if let Some(cond) = condition {
                session.set_breakpoint_condition(bp, &cond)?;
            }
            println!("Breakpoint {} set at {}", bp, location);
        }
        Command::Delete { number } => {
            session.delete_breakpoint(number)?;
            println!("Breakpoint {} deleted", number);
        }
        Command::Where => {
            println!("Current location: {}", session.current_location()?);
        }
        Command::Backtrace => {
            for (i, frame) in session.backtrace()?.iter().enumerate() {
                println!("#{}  {}", i, frame);
            }
        }
        Command::Print { expr } => {
            println!("{} = {}", expr, session.evaluate(&expr)?);
        }
        Command::List { location } => {
            for l in session.list_source(location.as_deref())? {
                println!("{}", l);
            }
        }
        Command::Info { item } => {
            println!("{}", session.get_info(&item)?);
        }
        Command::Attach { pid } => {
            let new_target = target::attach(pid)?;
            session.set_target(new_target)?;
            println!("Attached to process {}", pid);
        }
        Command::Detach => {
            session.detach()?;
            println!("Detached from target");
        }
        Command::Kill => {
            session.kill()?;
            println!("Process killed");
        }
        Command::Help => print_help(),
        Command::Quit => {
            let _ = session.detach();
            return Ok(LineOutcome::Quit);
        }
    }
    Ok(LineOutcome::Continue)
}

/// Print the command reference.
pub fn print_help() {
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
  print (p)        - Print variable/expression value
  list (l)         - List source code

  info             - Show info (threads, breakpoints, registers)
  attach           - Attach to running process
  detach           - Detach from process
  kill             - Kill debugged process

  quit (q)         - Exit debugger
  help (h)         - Show this help message"#
    );
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_commands() {
        assert_eq!(parse("continue").unwrap(), Command::Continue);
        assert_eq!(parse("c").unwrap(), Command::Continue);
        assert_eq!(parse("bt").unwrap(), Command::Backtrace);
        assert_eq!(parse("help").unwrap(), Command::Help);
        assert_eq!(parse("q").unwrap(), Command::Quit);
    }

    #[test]
    fn parses_run_with_args() {
        assert_eq!(
            parse("run foo bar").unwrap(),
            Command::Run { args: vec!["foo".into(), "bar".into()] }
        );
    }

    #[test]
    fn parses_break_with_condition() {
        let cmd = parse("b main.c:42 x > 3").unwrap();
        assert_eq!(
            cmd,
            Command::Break {
                location: "main.c:42".into(),
                condition: Some("x > 3".into()),
            }
        );
    }

    #[test]
    fn parses_print_multiword_expression() {
        assert_eq!(
            parse("p self count").unwrap(),
            Command::Print { expr: "self count".into() }
        );
    }

    #[test]
    fn rejects_unknown_and_malformed() {
        assert!(parse("frobnicate").is_err());
        assert!(parse("").is_err());
        assert!(parse("break").is_err());
        assert!(parse("delete notanumber").is_err());
        assert!(parse("print").is_err());
    }
}
