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

/// Variable information
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub type_name: String,
    pub children: Vec<Variable>,
}

/// Debug session
pub struct DebugSession {
    target: DebugTarget,
    state: State,
    breakpoints: HashMap<u32, Breakpoint>,
    next_bp_id: u32,
    current_location: Option<String>,
    stack_frames: Vec<StackFrame>,
    variables: HashMap<String, Variable>,
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
            variables: HashMap::new(),
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
    pub fn run(&mut self, args: &[String]) -> Result<(), DebuggerError> {
        if !matches!(self.target, DebugTarget::Binary { .. }) && !self.target.is_running() {
            return Err(DebuggerError::ProcessNotRunning);
        }
        
        println!("Starting program with args: {:?}", args);
        self.state = State::Running;
        
        // In production, this would spawn the process and wait for stop
        self.current_location = Some("main:1".to_string());
        self.state = State::Stopped(self.current_location.clone().unwrap());
        
        Ok(())
    }
    
    /// Continue execution until breakpoint
    pub fn continue_execution(&mut self) -> Result<(), DebuggerError> {
        if matches!(self.state, State::Idle | State::Exited(_)) {
            return Err(DebuggerError::ProcessNotRunning);
        }
        
        println!("Continuing execution...");
        self.state = State::Running;
        
        // Simulate hitting a breakpoint
        // In production, this would wait for the target to stop
        self.current_location = Some("main:10".to_string());
        self.state = State::Stopped(self.current_location.clone().unwrap());
        
        Ok(())
    }
    
    /// Step into function
    pub fn step_into(&mut self) -> Result<(), DebuggerError> {
        if !matches!(self.state, State::Stopped(_)) {
            return Err(DebuggerError::ProcessNotRunning);
        }
        
        println!("Stepping into...");
        // In production, send step command to target
        self.current_location = Some("helper_func:5".to_string());
        self.state = State::Stopped(self.current_location.clone().unwrap());
        
        Ok(())
    }
    
    /// Step over function
    pub fn step_over(&mut self) -> Result<(), DebuggerError> {
        if !matches!(self.state, State::Stopped(_)) {
            return Err(DebuggerError::ProcessNotRunning);
        }
        
        println!("Stepping over...");
        self.current_location = Some("main:15".to_string());
        self.state = State::Stopped(self.current_location.clone().unwrap());
        
        Ok(())
    }
    
    /// Step out of current function
    pub fn step_out(&mut self) -> Result<(), DebuggerError> {
        if !matches!(self.state, State::Stopped(_)) {
            return Err(DebuggerError::ProcessNotRunning);
        }
        
        println!("Stepping out...");
        self.current_location = Some("main:20".to_string());
        self.state = State::Stopped(self.current_location.clone().unwrap());
        
        Ok(())
    }
    
    /// Set a breakpoint
    pub fn set_breakpoint(&mut self, location: &str) -> Result<u32, DebuggerError> {
        let id = self.next_bp_id;
        self.next_bp_id += 1;
        
        let bp = Breakpoint {
            id,
            location: location.to_string(),
            address: None, // Would be resolved in production
            enabled: true,
            condition: None,
            hit_count: 0,
        };
        
        self.breakpoints.insert(id, bp);
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
    pub fn current_location(&self) -> Result<String, DebuggerError> {
        self.current_location.clone().ok_or_else(|| {
            DebuggerError::Target("No current location available".to_string())
        })
    }
    
    /// Get backtrace
    pub fn backtrace(&self) -> Result<Vec<StackFrame>, DebuggerError> {
        // Return mock stack frames
        Ok(vec![
            StackFrame {
                index: 0,
                function: "current_function".to_string(),
                file: Some("main.nova".to_string()),
                line: Some(10),
                address: 0x400100,
            },
            StackFrame {
                index: 1,
                function: "caller_function".to_string(),
                file: Some("main.nova".to_string()),
                line: Some(25),
                address: 0x400200,
            },
            StackFrame {
                index: 2,
                function: "main".to_string(),
                file: Some("main.nova".to_string()),
                line: Some(5),
                address: 0x400000,
            },
        ])
    }
    
    /// Evaluate expression
    pub fn evaluate(&self, expr: &str) -> Result<String, DebuggerError> {
        // Mock evaluation
        Ok(format!("<evaluated: {}>", expr))
    }
    
    /// List source code
    pub fn list_source(&self, _location: Option<&str>) -> Result<Vec<String>, DebuggerError> {
        // Return mock source lines
        Ok(vec![
            "1: fn main() {".to_string(),
            "2:     var x = 10;".to_string(),
            "3:     var y = 20;".to_string(),
            "4:     print(x + y);".to_string(),
            "5: }".to_string(),
        ])
    }
    
    /// Detach from target
    pub fn detach(&mut self) -> Result<(), DebuggerError> {
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
            "variables" => {
                if self.variables.is_empty() {
                    Ok("No variables in scope".to_string())
                } else {
                    let mut info = String::new();
                    for (_, var) in &self.variables {
                        info.push_str(&format!("{}: {} = {}\n", var.name, var.type_name, var.value));
                    }
                    Ok(info)
                }
            }
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
        
        // Add breakpoint
        let bp_id = session.set_breakpoint("main:10").unwrap();
        assert_eq!(bp_id, 1);
        
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
    fn test_backtrace() {
        let target = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        let session = DebugSession::new(target).unwrap();
        
        let frames = session.backtrace().unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].function, "current_function");
    }
}
