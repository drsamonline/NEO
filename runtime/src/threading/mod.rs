//! Threading Primitives for NOVA Runtime
//! 
//! Provides mutexes, condition variables, and synchronization primitives.

use std::sync::{Mutex, RwLock, Condvar, Arc};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle, ThreadId};
use std::time::Duration;
use crate::error::NovaError;

/// Result type for threading operations
pub type ThreadResult<T> = Result<T, NovaError>;

/// A managed mutex with error handling
pub struct NovaMutex<T> {
    inner: Mutex<T>,
    name: Option<String>,
    locked: AtomicBool,
}

impl<T> NovaMutex<T> {
    pub fn new(data: T) -> Self {
        NovaMutex {
            inner: Mutex::new(data),
            name: None,
            locked: AtomicBool::new(false),
        }
    }
    
    pub fn with_name(data: T, name: impl Into<String>) -> Self {
        NovaMutex {
            inner: Mutex::new(data),
            name: Some(name.into()),
            locked: AtomicBool::new(false),
        }
    }
    
    pub fn lock(&self) -> ThreadResult<MutexGuard<T>> {
        self.locked.store(true, Ordering::SeqCst);
        let guard = self.inner.lock()
            .map_err(|e| NovaError::sync_error(format!("Mutex poison: {:?}", e)))?;
        Ok(MutexGuard { _guard: guard, locked: &self.locked })
    }
    
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        match self.inner.try_lock() {
            Ok(guard) => {
                self.locked.store(true, Ordering::SeqCst);
                Some(MutexGuard { _guard: guard, locked: &self.locked })
            },
            Err(_) => None,
        }
    }
    
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }
    
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

pub struct MutexGuard<'a, T> {
    _guard: std::sync::MutexGuard<'a, T>,
    locked: &'a AtomicBool,
}

impl<'a, T> std::ops::Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target { &self._guard }
}

impl<'a, T> std::ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self._guard }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.locked.store(false, Ordering::SeqCst);
    }
}

/// A read-write lock with error handling
pub struct NovaRwLock<T> {
    inner: RwLock<T>,
    name: Option<String>,
    readers: AtomicUsize,
    writer: AtomicBool,
}

impl<T> NovaRwLock<T> {
    pub fn new(data: T) -> Self {
        NovaRwLock {
            inner: RwLock::new(data),
            name: None,
            readers: AtomicUsize::new(0),
            writer: AtomicBool::new(false),
        }
    }
    
    pub fn read(&self) -> ThreadResult<RwLockReadGuard<T>> {
        self.readers.fetch_add(1, Ordering::SeqCst);
        let guard = self.inner.read()
            .map_err(|e| NovaError::sync_error(format!("RWLock poison: {:?}", e)))?;
        Ok(RwLockReadGuard { _guard: guard, readers: &self.readers })
    }
    
    pub fn write(&self) -> ThreadResult<RwLockWriteGuard<T>> {
        self.writer.store(true, Ordering::SeqCst);
        let guard = self.inner.write()
            .map_err(|e| NovaError::sync_error(format!("RWLock poison: {:?}", e)))?;
        Ok(RwLockWriteGuard { _guard: guard, writer: &self.writer })
    }
    
    pub fn reader_count(&self) -> usize {
        self.readers.load(Ordering::SeqCst)
    }
    
    pub fn has_writer(&self) -> bool {
        self.writer.load(Ordering::SeqCst)
    }
}

pub struct RwLockReadGuard<'a, T> {
    _guard: std::sync::RwLockReadGuard<'a, T>,
    readers: &'a AtomicUsize,
}

impl<'a, T> std::ops::Deref for RwLockReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target { &self._guard }
}

impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        self.readers.fetch_sub(1, Ordering::SeqCst);
    }
}

pub struct RwLockWriteGuard<'a, T> {
    _guard: std::sync::RwLockWriteGuard<'a, T>,
    writer: &'a AtomicBool,
}

impl<'a, T> std::ops::Deref for RwLockWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target { &self._guard }
}

impl<'a, T> std::ops::DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self._guard }
}

impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.writer.store(false, Ordering::SeqCst);
    }
}

/// Condition variable
pub struct NovaCondVar {
    inner: Condvar,
    name: Option<String>,
}

impl NovaCondVar {
    pub fn new() -> Self {
        NovaCondVar {
            inner: Condvar::new(),
            name: None,
        }
    }
    
    pub fn notify_one(&self) {
        self.inner.notify_one();
    }
    
    pub fn notify_all(&self) {
        self.inner.notify_all();
    }
}

impl Default for NovaCondVar {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread builder
pub struct NovaThreadBuilder {
    name: Option<String>,
    stack_size: Option<usize>,
}

impl NovaThreadBuilder {
    pub fn new() -> Self {
        NovaThreadBuilder {
            name: None,
            stack_size: None,
        }
    }
    
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
    
    pub fn stack_size(mut self, size: usize) -> Self {
        self.stack_size = Some(size);
        self
    }
    
    pub fn spawn<F, T>(self, f: F) -> ThreadResult<NovaThread<T>>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let mut builder = thread::Builder::new();
        
        if let Some(name) = self.name {
            builder = builder.name(name);
        }
        
        if let Some(size) = self.stack_size {
            builder = builder.stack_size(size);
        }
        
        let handle = builder.spawn(f)
            .map_err(|e| NovaError::sync_error(format!("Failed to spawn thread: {}", e)))?;
        
        Ok(NovaThread {
            handle: Some(handle),
            joined: false,
        })
    }
}

impl Default for NovaThreadBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NovaThread<T> {
    handle: Option<JoinHandle<T>>,
    joined: bool,
}

impl<T> NovaThread<T> {
    pub fn join(&mut self) -> ThreadResult<T> {
        if self.joined {
            return Err(NovaError::sync_error("Thread already joined".to_string()));
        }
        
        let handle = self.handle.take()
            .ok_or_else(|| NovaError::sync_error("Thread handle not available".to_string()))?;
        
        self.joined = true;
        
        handle.join()
            .map_err(|_| NovaError::sync_error("Thread panicked".to_string()))
    }
    
    pub fn is_joined(&self) -> bool {
        self.joined
    }
    
    pub fn id(&self) -> Option<ThreadId> {
        self.handle.as_ref().map(|h| h.thread().id())
    }
}

pub fn spawn<F, T>(f: F) -> ThreadResult<NovaThread<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    NovaThreadBuilder::new().spawn(f)
}

pub fn current_thread_name() -> Option<String> {
    thread::current().name().map(|s| s.to_string())
}

pub fn sleep(duration: Duration) {
    thread::sleep(duration);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mutex_basic() {
        let mutex = NovaMutex::new(42);
        assert!(!mutex.is_locked());
        
        {
            let guard = mutex.lock().unwrap();
            assert!(mutex.is_locked());
            assert_eq!(*guard, 42);
        }
        
        assert!(!mutex.is_locked());
    }
    
    #[test]
    fn test_thread_spawn() {
        let mut thread = spawn(|| 42).unwrap();
        let result = thread.join().unwrap();
        assert_eq!(result, 42);
    }
}
