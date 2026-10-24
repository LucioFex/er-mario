//! Runs libsm64 on its own thread so a hang in SM64 code can never freeze the game for good.
//!
//! The game thread sends jobs and waits for each. A job slower than `TIMEOUT` is logged every
//! `STILL` while the game waits on it: Mario is never switched off for it (the user would rather
//! have the game stall a moment than lose Mario). A job that panics loses only its own result
//! (`None`): the thread lives on. If the thread is ever gone, the next call starts a new one and
//! `generation()` moves on, so lib.rs rebuilds Mario on it.
//!
//! `TIMEOUT` was 100 ms once. A PC that stalls while the world loads in can keep the thread
//! waiting longer than that with nothing wrong in SM64. At 3 s it once switched Mario off for the
//! whole session ("libsm64 HUNG"): now it only says so.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::log;
use crate::sm64::Geometry;

const TIMEOUT: Duration = Duration::from_secs(3);
/// once past its timeout, a job still running is logged this often
const STILL: Duration = Duration::from_secs(5);
/// a call this long is worth a line in the log
const SLOW: Duration = Duration::from_millis(100);

pub struct Ctx {
    pub geo: Geometry,
}

type Job = Box<dyn FnOnce(&mut Ctx) + Send>;

static SENDER: Mutex<Option<Sender<Job>>> = Mutex::new(None);
/// how many times the thread was started (a new one: a new SM64 context, lib.rs rebuilds Mario)
static GENERATION: AtomicU32 = AtomicU32::new(0);

fn spawn() -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    std::thread::spawn(move || {
        let mut ctx = Ctx { geo: Geometry::new() };
        while let Ok(job) = rx.recv() {
            // (a panic in a job drops its reply: that call alone returns None)
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&mut ctx))).is_err() {
                log("libsm64 thread: a job panicked (see the PANIC line above), the thread goes on");
            }
        }
    });
    GENERATION.fetch_add(1, Ordering::Relaxed);
    tx
}

/// Sends a job to the thread, starting one if there is none (or it's gone).
fn send(job: Job) -> bool {
    let mut sender = SENDER.lock().unwrap_or_else(|e| e.into_inner());
    let first = sender.is_none();
    let tx = sender.get_or_insert_with(spawn);
    let Err(mpsc::SendError(job)) = tx.send(job) else { return true };
    if !first {
        log("libsm64 thread was gone: started a new one (Mario is rebuilt on it)");
    }
    let tx = sender.insert(spawn());
    tx.send(job).is_ok()
}

/// The thread's generation: it changes when a new thread (and SM64 context) took over.
pub fn generation() -> u32 {
    GENERATION.load(Ordering::Relaxed)
}

/// Runs `f` on the libsm64 thread and waits for its result (None: it panicked).
pub fn call<R: Send + 'static>(name: &'static str, f: impl FnOnce(&mut Ctx) -> R + Send + 'static) -> Option<R> {
    call_timeout(name, TIMEOUT, f)
}

/// As `call`; past `timeout` the wait goes on, logged every `STILL`.
pub fn call_timeout<R: Send + 'static>(
    name: &'static str,
    timeout: Duration,
    f: impl FnOnce(&mut Ctx) -> R + Send + 'static,
) -> Option<R> {
    let (rtx, rrx) = mpsc::sync_channel(1);
    let job: Job = Box::new(move |ctx| {
        let _ = rtx.send(f(ctx));
    });
    if !send(job) {
        return None;
    }
    let sent = Instant::now();
    let mut wait = timeout;
    loop {
        match rrx.recv_timeout(wait) {
            Ok(r) => {
                let took = sent.elapsed();
                if took > timeout {
                    log(format!("libsm64 back from `{name}` after {} ms: Mario carries on", took.as_millis()));
                } else if took > SLOW && timeout <= TIMEOUT {
                    log(format!("libsm64 slow in `{name}`: {} ms", took.as_millis()));
                }
                return Some(r);
            }
            Err(RecvTimeoutError::Timeout) => {
                log(format!("libsm64 still in `{name}` after {:.1} s (the game waits for it)", sent.elapsed().as_secs_f32()));
                wait = STILL;
            }
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panicking_job_only_loses_itself() {
        assert!(call("boom", |_| -> i32 { panic!("test panic") }).is_none());
        assert_eq!(call("after", |_| 7), Some(7));
    }

    #[test]
    fn slow_job_is_waited_for() {
        let r = call_timeout("slow", Duration::from_millis(30), |_| {
            std::thread::sleep(Duration::from_millis(120));
            5
        });
        assert_eq!(r, Some(5));
        assert_eq!(call("after slow", |_| 6), Some(6));
    }
}
