//! Per-command cooperative cancellation. No ambient limit for library callers unless installed.
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Control {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl Control {
    pub fn new(timeout: Duration) -> Self {
        Self {
            deadline: Instant::now()
                .checked_add(timeout)
                .expect("bounded command timeout"),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<(), &'static str> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("command cancelled")
        } else if Instant::now() >= self.deadline {
            Err("command deadline exceeded")
        } else {
            Ok(())
        }
    }
    pub fn enter(&self) -> Scope {
        Scope {
            previous: CURRENT.with(|c| c.replace(Some(self.clone()))),
            same_thread: std::marker::PhantomData,
        }
    }
}
thread_local! { static CURRENT: RefCell<Option<Control>> = const { RefCell::new(None) }; }
pub struct Scope {
    previous: Option<Control>,
    same_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|c| c.replace(self.previous.take()));
    }
}
pub fn current() -> Option<Control> {
    CURRENT.with(|c| c.borrow().clone())
}
pub fn check() -> Result<(), &'static str> {
    CURRENT.with(|c| c.borrow().as_ref().map_or(Ok(()), Control::check))
}
pub fn stopped() -> bool {
    check().is_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_scopes_restore_and_cancellation_crosses_threads() {
        let outer = Control::new(Duration::from_secs(30));
        let _scope = outer.enter();
        {
            let inner = Control::new(Duration::ZERO);
            let _inner = inner.enter();
            assert_eq!(check(), Err("command deadline exceeded"));
        }
        assert!(check().is_ok());
        let token = outer.clone();
        std::thread::spawn(move || token.cancel()).join().unwrap();
        assert_eq!(check(), Err("command cancelled"));
    }
    #[test]
    fn deadline_remains_expired_and_scope_restores_unlimited_library_behavior() {
        {
            let c = Control::new(Duration::ZERO);
            let _scope = c.enter();
            assert!(stopped());
            assert!(current().unwrap().check().is_err());
        }
        assert!(check().is_ok());
    }
}
