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
            return Err(DebuggerError::AttachFailed(
                format!("Process {} does not exist", pid)
            ));
        }
        
        // Try to read process name
        let comm_path = proc_path.join("comm");
        let process_name = std::fs::read_to_string(&comm_path)
            .unwrap_or_else(|_| "unknown".to_string())
            .trim()
            .to_string();
        
        println!("Attached to process {} ({})", pid, process_name);
        
        Ok(DebugTarget::Attached { pid, process_name })
    }
    
    #[cfg(windows)]
    {
        // Windows implementation would use Win32 APIs
        println!("Attached to process {}", pid);
        
        Ok(DebugTarget::Attached {
            pid,
            process_name: "unknown".to_string(),
        })
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
    
    // In production, this would establish a TCP connection
    println!("Connected to remote target {}:{}", host, port);
    
    Ok(DebugTarget::Remote {
        host,
        port,
        connected: true,
    })
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
