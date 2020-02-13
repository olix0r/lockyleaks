use futures::{Async, future, try_ready, Future, Poll};
use std::time::Duration;
use tokio::sync::lock;

fn main() {
    tokio::run(future::lazy(|| {
        let lock = lock::Lock::new(());

        for _ in 0..10_000 {
            let fut = Loop {
                lock: lock.clone(),
                pending: None,
            };
            tokio::spawn(fut);
        }

        future::empty()
    }));
}

struct Loop {
    lock: lock::Lock<()>,
    pending: Option<tokio::timer::Timeout<AcquireAndIdle>>,
}

struct AcquireAndIdle {
    lock: lock::Lock<()>,
    locked: Option<lock::LockGuard<()>>,
}

impl Future for AcquireAndIdle {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            if self.locked.is_some() {
                return Ok(Async::NotReady);
            }

            let guard = try_ready!(Ok(self.lock.poll_lock()));
            self.locked = Some(guard);
        }
    }
}

impl Future for Loop {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            if let Some(pending) = self.pending.as_mut() {
                if let Ok(Async::NotReady) = pending.poll() {
                    return Ok(Async::NotReady);
                }
                self.pending = None;
            }

            let fut = AcquireAndIdle {
                lock: self.lock.clone(),
                locked: None,
            };
            self.pending = Some(tokio::timer::Timeout::new(fut, Duration::from_millis(1000)));
        }
    }
}
