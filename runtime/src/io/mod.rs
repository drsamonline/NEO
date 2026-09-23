//! I/O Runtime Support for NOVA
//! 
//! Provides file I/O, streams, and buffered operations for the NOVA runtime.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use crate::error::NovaError;

/// Result type for I/O operations
pub type IoResult<T> = Result<T, NovaError>;

/// File access modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Read,
    Write,
    Append,
    ReadWrite,
    ReadWriteCreate,
    Create,
    Truncate,
}

impl FileMode {
    pub fn to_open_options(&self) -> OpenOptions {
        let mut opts = OpenOptions::new();
        match self {
            FileMode::Read => { opts.read(true); },
            FileMode::Write => { opts.write(true).create(true).truncate(true); },
            FileMode::Append => { opts.write(true).append(true).create(true); },
            FileMode::ReadWrite => { opts.read(true).write(true); },
            FileMode::ReadWriteCreate => { opts.read(true).write(true).create(true); },
            FileMode::Create => { opts.write(true).create_new(true); },
            FileMode::Truncate => { opts.write(true).truncate(true); },
        }
        opts
    }
}

/// Buffered stream types
#[derive(Debug)]
pub enum StreamType {
    Input,
    Output,
    InputOutput,
}

/// A managed file handle with buffering
pub struct NovaFile {
    path: PathBuf,
    mode: FileMode,
    stream_type: StreamType,
    inner: FileHandleInner,
    is_closed: bool,
}

enum FileHandleInner {
    Reader(BufReader<File>),
    Writer(BufWriter<File>),
    ReadWriter { reader: BufReader<File>, writer: BufWriter<File> },
}

impl NovaFile {
    /// Open a file with the specified mode
    pub fn open<P: AsRef<Path>>(path: P, mode: FileMode) -> IoResult<Self> {
        let path = path.as_ref().to_path_buf();
        let opts = mode.to_open_options();
        
        let file = opts.open(&path)
            .map_err(|e| NovaError::io_error(format!("Failed to open '{}': {}", path.display(), e)))?;
        
        let (inner, stream_type) = match mode {
            FileMode::Read => (FileHandleInner::Reader(BufReader::new(file)), StreamType::Input),
            FileMode::Write | FileMode::Append | FileMode::Create | FileMode::Truncate => {
                (FileHandleInner::Writer(BufWriter::new(file)), StreamType::Output)
            },
            FileMode::ReadWrite | FileMode::ReadWriteCreate => {
                let reader = BufReader::new(file.try_clone()
                    .map_err(|e| NovaError::io_error(format!("Failed to clone file handle: {}", e)))?);
                let writer = BufWriter::new(file);
                (FileHandleInner::ReadWriter { reader, writer }, StreamType::InputOutput)
            },
        };
        
        Ok(NovaFile {
            path,
            mode,
            stream_type,
            inner,
            is_closed: false,
        })
    }
    
    /// Read bytes from the file
    pub fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.is_closed {
            return Err(NovaError::io_error("Cannot read from closed file".to_string()));
        }
        
        match &mut self.inner {
            FileHandleInner::Reader(reader) => {
                reader.read(buf)
                    .map_err(|e| NovaError::io_error(format!("Read error: {}", e)))
            },
            FileHandleInner::ReadWriter { reader, .. } => {
                reader.read(buf)
                    .map_err(|e| NovaError::io_error(format!("Read error: {}", e)))
            },
            _ => Err(NovaError::io_error("File not opened for reading".to_string())),
        }
    }
    
    /// Read a line from the file
    pub fn read_line(&mut self) -> IoResult<Option<String>> {
        let mut buf = String::new();
        match &mut self.inner {
            FileHandleInner::Reader(reader) => {
                let bytes_read = reader.read_line(&mut buf)
                    .map_err(|e| NovaError::io_error(format!("Read line error: {}", e)))?;
                if bytes_read == 0 {
                    Ok(None)
                } else {
                    Ok(Some(buf))
                }
            },
            FileHandleInner::ReadWriter { reader, .. } => {
                let bytes_read = reader.read_line(&mut buf)
                    .map_err(|e| NovaError::io_error(format!("Read line error: {}", e)))?;
                if bytes_read == 0 {
                    Ok(None)
                } else {
                    Ok(Some(buf))
                }
            },
            _ => Err(NovaError::io_error("File not opened for reading".to_string())),
        }
    }
    
    /// Write bytes to the file
    pub fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.is_closed {
            return Err(NovaError::io_error("Cannot write to closed file".to_string()));
        }
        
        match &mut self.inner {
            FileHandleInner::Writer(writer) => {
                writer.write(buf)
                    .map_err(|e| NovaError::io_error(format!("Write error: {}", e)))
            },
            FileHandleInner::ReadWriter { writer, .. } => {
                writer.write(buf)
                    .map_err(|e| NovaError::io_error(format!("Write error: {}", e)))
            },
            _ => Err(NovaError::io_error("File not opened for writing".to_string())),
        }
    }
    
    /// Write a string to the file
    pub fn write_str(&mut self, s: &str) -> IoResult<usize> {
        self.write(s.as_bytes())
    }
    
    /// Flush the buffer
    pub fn flush(&mut self) -> IoResult<()> {
        match &mut self.inner {
            FileHandleInner::Writer(writer) => {
                writer.flush()
                    .map_err(|e| NovaError::io_error(format!("Flush error: {}", e)))
            },
            FileHandleInner::ReadWriter { writer, .. } => {
                writer.flush()
                    .map_err(|e| NovaError::io_error(format!("Flush error: {}", e)))
            },
            _ => Ok(()),
        }
    }
    
    /// Seek to a position
    pub fn seek(&mut self, pos: SeekFrom) -> IoResult<u64> {
        if self.is_closed {
            return Err(NovaError::io_error("Cannot seek in closed file".to_string()));
        }
        
        self.flush()?;
        
        match &mut self.inner {
            FileHandleInner::Reader(reader) => {
                reader.seek(pos)
                    .map_err(|e| NovaError::io_error(format!("Seek error: {}", e)))
            },
            FileHandleInner::Writer(writer) => {
                writer.seek(pos)
                    .map_err(|e| NovaError::io_error(format!("Seek error: {}", e)))
            },
            FileHandleInner::ReadWriter { reader, writer } => {
                let pos_reader = reader.seek(pos)
                    .map_err(|e| NovaError::io_error(format!("Seek error (reader): {}", e)))?;
                writer.seek(pos)
                    .map_err(|e| NovaError::io_error(format!("Seek error (writer): {}", e)))?;
                Ok(pos_reader)
            },
        }
    }
    
    /// Get current position
    pub fn position(&mut self) -> IoResult<u64> {
        self.seek(SeekFrom::Current(0))
    }
    
    /// Get file size
    pub fn size(&self) -> IoResult<u64> {
        let metadata = std::fs::metadata(&self.path)
            .map_err(|e| NovaError::io_error(format!("Failed to get metadata: {}", e)))?;
        Ok(metadata.len())
    }
    
    /// Close the file
    pub fn close(&mut self) -> IoResult<()> {
        if self.is_closed {
            return Ok(());
        }
        
        self.flush()?;
        self.is_closed = true;
        Ok(())
    }
    
    /// Check if file is closed
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }
    
    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }
    
    /// Get the file mode
    pub fn mode(&self) -> FileMode {
        self.mode
    }
}

impl Drop for NovaFile {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Standard input wrapper
pub struct StdIn {
    inner: BufReader<std::io::Stdin>,
}

impl StdIn {
    pub fn new() -> Self {
        StdIn {
            inner: BufReader::new(std::io::stdin()),
        }
    }
    
    pub fn read_line(&mut self) -> IoResult<Option<String>> {
        let mut buf = String::new();
        let bytes = self.inner.read_line(&mut buf)
            .map_err(|e| NovaError::io_error(format!("stdin read error: {}", e)))?;
        if bytes == 0 {
            Ok(None)
        } else {
            Ok(Some(buf))
        }
    }
    
    pub fn read_char(&mut self) -> IoResult<Option<char>> {
        let mut buf = [0u8; 1];
        let bytes = self.inner.read(&mut buf)
            .map_err(|e| NovaError::io_error(format!("stdin read error: {}", e)))?;
        if bytes == 0 {
            Ok(None)
        } else {
            Ok(Some(buf[0] as char))
        }
    }
}

impl Default for StdIn {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard output wrapper
pub struct StdOut {
    inner: BufWriter<std::io::Stdout>,
}

impl StdOut {
    pub fn new() -> Self {
        StdOut {
            inner: BufWriter::new(std::io::stdout()),
        }
    }
    
    pub fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.inner.write(buf)
            .map_err(|e| NovaError::io_error(format!("stdout write error: {}", e)))
    }
    
    pub fn write_str(&mut self, s: &str) -> IoResult<usize> {
        self.write(s.as_bytes())
    }
    
    pub fn flush(&mut self) -> IoResult<()> {
        self.inner.flush()
            .map_err(|e| NovaError::io_error(format!("stdout flush error: {}", e)))
    }
}

impl Default for StdOut {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard error wrapper
pub struct StdErr {
    inner: BufWriter<std::io::Stderr>,
}

impl StdErr {
    pub fn new() -> Self {
        StdErr {
            inner: BufWriter::new(std::io::stderr()),
        }
    }
    
    pub fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.inner.write(buf)
            .map_err(|e| NovaError::io_error(format!("stderr write error: {}", e)))
    }
    
    pub fn write_str(&mut self, s: &str) -> IoResult<usize> {
        self.write(s.as_bytes())
    }
    
    pub fn flush(&mut self) -> IoResult<()> {
        self.inner.flush()
            .map_err(|e| NovaError::io_error(format!("stderr flush error: {}", e)))
    }
}

impl Default for StdErr {
    fn default() -> Self {
        Self::new()
    }
}

/// Directory operations
pub mod dir {
    use super::*;
    use std::fs;
    
    /// List directory contents
    pub fn list<P: AsRef<Path>>(path: P) -> IoResult<Vec<PathBuf>> {
        let entries = fs::read_dir(path)
            .map_err(|e| NovaError::io_error(format!("Failed to read directory: {}", e)))?;
        
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry
                .map_err(|e| NovaError::io_error(format!("Failed to read entry: {}", e)))?;
            paths.push(entry.path());
        }
        
        Ok(paths)
    }
    
    /// Create a directory
    pub fn create<P: AsRef<Path>>(path: P) -> IoResult<()> {
        fs::create_dir(path)
            .map_err(|e| NovaError::io_error(format!("Failed to create directory: {}", e)))
    }
    
    /// Create directories recursively
    pub fn create_all<P: AsRef<Path>>(path: P) -> IoResult<()> {
        fs::create_dir_all(path)
            .map_err(|e| NovaError::io_error(format!("Failed to create directories: {}", e)))
    }
    
    /// Remove a directory
    pub fn remove<P: AsRef<Path>>(path: P) -> IoResult<()> {
        fs::remove_dir(path)
            .map_err(|e| NovaError::io_error(format!("Failed to remove directory: {}", e)))
    }
    
    /// Remove a directory and all its contents
    pub fn remove_all<P: AsRef<Path>>(path: P) -> IoResult<()> {
        fs::remove_dir_all(path)
            .map_err(|e| NovaError::io_error(format!("Failed to remove directory: {}", e)))
    }
}

/// File operations
pub mod file {
    use super::*;
    use std::fs;
    
    /// Check if a path exists
    pub fn exists<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().exists()
    }
    
    /// Check if path is a file
    pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().is_file()
    }
    
    /// Check if path is a directory
    pub fn is_dir<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().is_dir()
    }
    
    /// Get file metadata size
    pub fn size<P: AsRef<Path>>(path: P) -> IoResult<u64> {
        let metadata = fs::metadata(path)
            .map_err(|e| NovaError::io_error(format!("Failed to get metadata: {}", e)))?;
        Ok(metadata.len())
    }
    
    /// Copy a file
    pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> IoResult<u64> {
        fs::copy(from, to)
            .map_err(|e| NovaError::io_error(format!("Failed to copy file: {}", e)))
    }
    
    /// Move/rename a file
    pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> IoResult<()> {
        fs::rename(from, to)
            .map_err(|e| NovaError::io_error(format!("Failed to rename file: {}", e)))
    }
    
    /// Remove a file
    pub fn remove<P: AsRef<Path>>(path: P) -> IoResult<()> {
        fs::remove_file(path)
            .map_err(|e| NovaError::io_error(format!("Failed to remove file: {}", e)))
    }
    
    /// Read entire file contents as bytes
    pub fn read_bytes<P: AsRef<Path>>(path: P) -> IoResult<Vec<u8>> {
        fs::read(path)
            .map_err(|e| NovaError::io_error(format!("Failed to read file: {}", e)))
    }
    
    /// Read entire file contents as string
    pub fn read_string<P: AsRef<Path>>(path: P) -> IoResult<String> {
        fs::read_to_string(path)
            .map_err(|e| NovaError::io_error(format!("Failed to read file: {}", e)))
    }
    
    /// Write bytes to a file
    pub fn write_bytes<P: AsRef<Path>>(path: P, contents: &[u8]) -> IoResult<()> {
        fs::write(path, contents)
            .map_err(|e| NovaError::io_error(format!("Failed to write file: {}", e)))
    }
    
    /// Write string to a file
    pub fn write_string<P: AsRef<Path>>(path: P, contents: &str) -> IoResult<()> {
        write_bytes(path, contents.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::SeekFrom;
    
    #[test]
    fn test_file_write_read() {
        let test_path = "/tmp/nova_test_file.txt";
        
        {
            let mut file = NovaFile::open(test_path, FileMode::Write).unwrap();
            assert_eq!(file.write_str("Hello, NOVA!").unwrap(), 12);
            file.close().unwrap();
        }
        
        {
            let mut file = NovaFile::open(test_path, FileMode::Read).unwrap();
            let mut buf = [0u8; 20];
            let bytes_read = file.read(&mut buf).unwrap();
            assert_eq!(bytes_read, 12);
            assert_eq!(&buf[..bytes_read], b"Hello, NOVA!");
            file.close().unwrap();
        }
        
        let _ = file::remove(test_path);
    }
    
    #[test]
    fn test_file_read_line() {
        let test_path = "/tmp/nova_test_lines.txt";
        
        {
            let mut file = NovaFile::open(test_path, FileMode::Write).unwrap();
            file.write_str("Line 1\n").unwrap();
            file.write_str("Line 2\n").unwrap();
            file.write_str("Line 3").unwrap();
            file.close().unwrap();
        }
        
        {
            let mut file = NovaFile::open(test_path, FileMode::Read).unwrap();
            assert_eq!(file.read_line().unwrap(), Some("Line 1\n".to_string()));
            assert_eq!(file.read_line().unwrap(), Some("Line 2\n".to_string()));
            assert_eq!(file.read_line().unwrap(), Some("Line 3".to_string()));
            assert_eq!(file.read_line().unwrap(), None);
            file.close().unwrap();
        }
        
        let _ = file::remove(test_path);
    }
    
    #[test]
    fn test_std_streams() {
        let mut stdout = StdOut::new();
        let mut stderr = StdErr::new();
        
        assert!(stderr.write_str("Test error message\n").is_ok());
        stderr.flush().unwrap();
        
        assert!(stdout.write_str("Test output message\n").is_ok());
        stdout.flush().unwrap();
    }
}
