//! Debug target management
//! 
//! Handles loading binaries, attaching to processes, and remote debugging connections.

use crate::DebuggerError;
use std::path::Path;

/// Represents a debug target
#[derive(Debug)]
pub enum DebugTarget {
    /// Local binary to run
    Binary {
        path: String,
        symbols_loaded: bool,
    },
    /// Attached to running process
    Attached {
        pid: u32,
        process_name: String,
    },
    /// Remote GDB stub connection
    Remote {
        host: String,
        port: u16,
        connected: bool,
    },
}

impl DebugTarget {
    pub fn is_running(&self) -> bool {
        match self {
            DebugTarget::Binary { .. } => false,
            DebugTarget::Attached { .. } => true,
            DebugTarget::Remote { connected, .. } => *connected,
        }
    }
    
    pub fn pid(&self) -> Option<u32> {
        match self {
            DebugTarget::Attached { pid, .. } => Some(*pid),
            _ => None,
        }
    }
}

/// Load a binary for debugging
pub fn load_binary(path: &Path) -> Result<DebugTarget, DebuggerError> {
    if !path.exists() {
        return Err(DebuggerError::Target(
            format!("Binary not found: {:?}", path)
        ));
    }
    
    // Verify it's an executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file() || (metadata.permissions().mode() & 0o111) == 0 {
            return Err(DebuggerError::Target(
                format!("Not an executable: {:?}", path)
            ));
        }
    }
    
    println!("Binary loaded: {:?}", path);
    
    Ok(DebugTarget::Binary {
        path: path.to_string_lossy().to_string(),
        symbols_loaded: false,
    })
}

/// Attach to a running process
pub fn attach(pid: u32) -> Result<DebugTarget, DebuggerError> {
    #[cfg(unix)]
    {
        // Check if process exists
        let proc_path = std::path::Path::new("/proc").join(pid.to_string());
        if !proc_path.exists() {
            return Err(DebuggerError::AttachFailed(format!(
                "process {pid} does not exist. Nothing was attached"
            )));
        }
        
        let comm_path = proc_path.join("comm");
        let process_name = std::fs::read_to_string(&comm_path)
            .unwrap_or_else(|_| "unknown".to_string())
            .trim()
            .to_string();

        // SAFETY: `ptrace` is variadic and takes pointers whose meaning depends on the request.
        // For `PTRACE_ATTACH` both are ignored, so passing null is correct; the kernel only
        // reads `pid`, and `pid` came from the caller as a `u32`, which is a valid `pid_t`.
        let attached = unsafe {
            libc::ptrace(
                libc::PTRACE_ATTACH,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
        if attached == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::AttachFailed(format!(
                "cannot attach to process {pid} ({process_name}): {error}. Nothing was attached"
            )));
        }

        // `PTRACE_ATTACH` sends `SIGSTOP`; the process is not ours to read until that stop is
        // reaped. Without this wait, a register read races the kernel and returns garbage.
        let mut status: libc::c_int = 0;
        // SAFETY: `waitpid` writes one `c_int` through the pointer, which points at a live
        // local of exactly that type, and `pid` is a valid `pid_t`.
        let waited = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, 0) };
        if waited == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::AttachFailed(format!(
                "attached to {pid} but could not wait for it to stop: {error}"
            )));
        }
        if !libc::WIFSTOPPED(status) {
            return Err(DebuggerError::AttachFailed(format!(
                "process {pid} did not stop after attach (status {status})"
            )));
        }

        Ok(DebugTarget::Attached { pid, process_name })
    }
    
    #[cfg(windows)]
    {
        // Windows would need `DebugActiveProcess`. Refused rather than reported as done.
        Err(DebuggerError::AttachFailed(format!(
            "process {pid}: attaching is not implemented on Windows in this debugger; \
             nothing was attached"
        )))
    }
}

/// Detach from a process this debugger attached to, letting it run again.
pub fn detach(pid: u32) -> Result<(), DebuggerError> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: as in `attach`, `PTRACE_DETACH` ignores both pointers; `pid` is the process
        // this debugger attached to, and detaching a process it did not attach to fails with
        // `EPERM` rather than doing something undefined.
        let detached = unsafe {
            libc::ptrace(
                libc::PTRACE_DETACH,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
        if detached == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!(
                "cannot detach from {pid}: {error}"
            )));
        }
        return Ok(());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        Err(DebuggerError::Target(
            "detaching is implemented on Linux only".to_string(),
        ))
    }
}

/// The instruction pointer of a stopped process — where it is right now.
///
/// x86-64 only. A stopped process's `rip` is the address of the instruction it will execute
/// next, which is exactly what `addr2line` needs to name a source line.
pub fn instruction_pointer(pid: u32) -> Result<u64, DebuggerError> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // SAFETY: `user_regs_struct` is plain data; zeroing it is a valid initial value and
        // `PTRACE_GETREGS` overwrites it. The pointer is to a live local of the exact type the
        // kernel writes, and `pid` is a process this debugger attached to.
        let mut registers: libc::user_regs_struct = unsafe { std::mem::zeroed() };
        let read = unsafe {
            libc::ptrace(
                libc::PTRACE_GETREGS,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                &mut registers as *mut libc::user_regs_struct as *mut libc::c_void,
            )
        };
        if read == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!(
                "cannot read registers of {pid}: {error}. Is it attached and stopped?"
            )));
        }
        Ok(registers.rip)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = pid;
        Err(DebuggerError::Target(
            "reading the instruction pointer is implemented for x86-64 Linux only".to_string(),
        ))
    }
}

/// Connect to a remote GDB stub
pub fn connect_remote(address: &str) -> Result<DebugTarget, DebuggerError> {
    let parts: Vec<&str> = address.split(':').collect();
    
    if parts.len() != 2 {
        return Err(DebuggerError::Target(
            "Remote address must be in format host:port".to_string()
        ));
    }
    
    let host = parts[0].to_string();
    let port: u16 = parts[1].parse().map_err(|_| {
        DebuggerError::Target("Invalid port number".to_string())
    })?;
    
    // No socket is opened here. This used to print "Connected to remote target host:port" and
    // return `connected: true`, so the debugger reported a connection that did not exist and
    // then had no way to use one.
    Err(DebuggerError::Target(format!(
        "cannot connect to {host}:{port}: the remote GDB stub protocol is not implemented. \
         Nothing was connected"
    )))
}

/// Get process memory map (platform-specific)
pub fn get_memory_map(pid: u32) -> Result<Vec<MemoryRegion>, DebuggerError> {
    #[cfg(unix)]
    {
        let maps_path = std::path::Path::new("/proc")
            .join(pid.to_string())
            .join("maps");
        
        let content = std::fs::read_to_string(&maps_path)?;
        let mut regions = Vec::new();
        
        for line in content.lines() {
            // Parse /proc/pid/maps format
            // Example: 00400000-00452000 r-xp 00000000 08:02 123456 /path/to/binary
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                let addr_range: Vec<&str> = parts[0].split('-').collect();
                if addr_range.len() == 2 {
                    let start = u64::from_str_radix(addr_range[0], 16).unwrap_or(0);
                    let end = u64::from_str_radix(addr_range[1], 16).unwrap_or(0);
                    let perms = parts[1].to_string();
                    let path = parts.get(5).map(|s| s.to_string()).unwrap_or_default();
                    
                    regions.push(MemoryRegion {
                        start,
                        end,
                        permissions: perms,
                        path,
                    });
                }
            }
        }
        
        Ok(regions)
    }
    
    #[cfg(not(unix))]
    {
        Ok(vec![])
    }
}

/// Memory region information
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub permissions: String,
    pub path: String,
}

impl MemoryRegion {
    pub fn size(&self) -> u64 {
        self.end - self.start
    }
    
    pub fn is_readable(&self) -> bool {
        self.permissions.contains('r')
    }
    
    pub fn is_writable(&self) -> bool {
        self.permissions.contains('w')
    }
    
    pub fn is_executable(&self) -> bool {
        self.permissions.contains('x')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_target_is_running() {
        let binary = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        assert!(!binary.is_running());
        
        let attached = DebugTarget::Attached {
            pid: 1234,
            process_name: "test".to_string(),
        };
        assert!(attached.is_running());
    }
    
    #[test]
    fn test_target_pid() {
        let binary = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        assert_eq!(binary.pid(), None);
        
        let attached = DebugTarget::Attached {
            pid: 1234,
            process_name: "test".to_string(),
        };
        assert_eq!(attached.pid(), Some(1234));
    }
    
    #[test]
    fn test_memory_region_permissions() {
        let region = MemoryRegion {
            start: 0x400000,
            end: 0x452000,
            permissions: "r-xp".to_string(),
            path: "/bin/test".to_string(),
        };
        
        assert!(region.is_readable());
        assert!(!region.is_writable());
        assert!(region.is_executable());
        assert_eq!(region.size(), 0x52000);
    }
}
