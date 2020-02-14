use futures::{future, try_ready, Async, Future, Poll, Stream};
use std::time::Duration;
use tokio::sync::{lock, mpsc};

fn main() {
    const USAGE: &'static str = "usage: lockyleaks <concurrency> <iterations>";
    let args = std::env::args().collect::<Vec<_>>();
    let concurrency = args
        .get(1)
        .expect(USAGE)
        .parse::<usize>()
        .expect("invalid concurrency");
    let iterations = args
        .get(2)
        .expect(USAGE)
        .parse::<usize>()
        .expect("invalid iterations");

    tokio::run(future::lazy(move || {
        let lock = lock::Lock::new(());

        let (tx, rx) = mpsc::channel(1);
        for _ in 0..concurrency {
            tokio::spawn(Loop {
                lock: lock.clone(),
                pending: None,
                remaining: iterations,
                _tx: tx.clone(),
            });
        }

        rx.fold((), |_, _| Ok(())).map_err(|_| panic!())
    }));
}

struct Loop {
    lock: lock::Lock<()>,
    pending: Option<tokio::timer::Timeout<AcquireAndIdle>>,
    remaining: usize,
    _tx: mpsc::Sender<()>,
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
                self.remaining -= 1;
                self.pending = None;
            }

            if self.remaining == 0 {
                return Ok(Async::Ready(()));
            }

            let fut = AcquireAndIdle {
                lock: self.lock.clone(),
                locked: None,
            };
            self.pending = Some(tokio::timer::Timeout::new(fut, Duration::from_millis(100)));
        }
    }
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
