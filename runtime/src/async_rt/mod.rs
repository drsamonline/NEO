//! Async Runtime for NOVA
//! 
//! Provides event loop, futures, and async/await support.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use crate::error::NovaError;

/// Result type for async operations
pub type AsyncResult<T> = Result<T, NovaError>;

/// Simple delay future
pub struct Delay {
    deadline: Option<Instant>,
    duration: Duration,
}

impl Delay {
    pub fn new(duration: Duration) -> Self {
        Delay {
            deadline: None,
            duration,
        }
    }
}

impl Future for Delay {
    type Output = ();
    
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.deadline {
            None => {
                self.deadline = Some(Instant::now() + self.duration);
                Poll::Pending
            },
            Some(deadline) if Instant::now() >= deadline => Poll::Ready(()),
            Some(_) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            },
        }
    }
}

pub fn delay(duration: Duration) -> Delay {
    Delay::new(duration)
}

/// Simple channel for async communication
pub struct Channel<T> {
    buffer: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<T>>>,
    closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Channel {
            buffer: Arc::clone(&self.buffer),
            closed: Arc::clone(&self.closed),
        }
    }
}

use std::sync::{Arc, Mutex};

impl<T> Channel<T> {
    pub fn new(capacity: usize) -> Self {
        Channel {
            buffer: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(capacity))),
            closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    
    pub fn send(&self, value: T) -> AsyncResult<()> {
        let mut buf = self.buffer.lock()
            .map_err(|e| NovaError::async_error(format!("Channel lock failed: {}", e)))?;
        buf.push_back(value);
        Ok(())
    }
    
    pub fn try_recv(&self) -> AsyncResult<Option<T>> {
        let mut buf = self.buffer.lock()
            .map_err(|e| NovaError::async_error(format!("Channel lock failed: {}", e)))?;
        Ok(buf.pop_front())
    }
    
    pub fn close(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    
    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_delay_creation() {
        let _delay = delay(Duration::from_millis(10));
    }
    
    #[test]
    fn test_channel_basic() {
        let ch = Channel::<i32>::new(10);
        ch.send(42).unwrap();
        assert_eq!(ch.try_recv().unwrap(), Some(42));
        assert_eq!(ch.try_recv().unwrap(), None);
    }
}
