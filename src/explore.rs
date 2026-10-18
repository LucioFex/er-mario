//! Safe memory access for reading the game's structures: readable-memory checks (with a small
//! cache of readable regions), raw reads and RTTI class names.

use fromsoftware_shared::UnknownPtr;
use windows::Win32::System::Memory::IsBadReadPtr;

/// Memory check statistics (perf log): checks, and how many said no.
pub static CHECKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static FAILED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Time the checks that said no took (ns): each is a fault Windows catches.
pub static FAILED_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// What one check costs here (µs) on memory that's there and on memory that isn't, timed over
/// a couple of hundred tries: a fault goes through every exception handler other mods put in.
pub fn bench() -> (f64, f64) {
    use windows::Win32::System::Memory::{VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_NOACCESS};
    const N: u32 = 200;
    let there = [0u64; 4];
    let t = std::time::Instant::now();
    for _ in 0..N {
        let _ = std::hint::black_box(unsafe { IsBadReadPtr(Some(there.as_ptr().cast()), 8) });
    }
    let ok = t.elapsed().as_secs_f64() * 1e6 / N as f64;
    let page = unsafe { VirtualAlloc(None, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_NOACCESS) };
    if page.is_null() {
        return (ok, -1.0);
    }
    let t = std::time::Instant::now();
    for _ in 0..N {
        let _ = std::hint::black_box(unsafe { IsBadReadPtr(Some(page.cast_const()), 8) });
    }
    let bad = t.elapsed().as_secs_f64() * 1e6 / N as f64;
    unsafe {
        let _ = VirtualFree(page, 0, MEM_RELEASE);
    }
    (ok, bad)
}

/// True if `len` bytes at `addr` can be read.
///
/// Windows tries the read itself (IsBadReadPtr: a byte of every page, any fault caught), which
/// costs next to nothing when the memory is there. This used to ask VirtualQuery and keep the
/// answers for a while, with a background thread asking again to keep them fresh. But
/// VirtualQuery sizes up the whole region around an address, and for the game's heaps, gigabytes
/// of them, that took 24 ms at a time on an ordinary PC (93 at worst) with the process's address
/// space held meanwhile: the game's own threads stood still for it. Measured as a 135 ms frame
/// about twice a second with all regions re-checked in one go, 34 ms frames still with the
/// checks spread out.
pub fn readable(addr: usize, len: usize) -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    if addr < 0x10000 || addr % 8 != 0 {
        return false;
    }
    CHECKS.fetch_add(1, Relaxed);
    let t = std::time::Instant::now();
    let probe = |at: usize, n: usize| !unsafe { IsBadReadPtr(Some(at as *const std::ffi::c_void), n) }.as_bool();
    // A big range (the body table is 5 MB, asked about on every collision query) is tried at
    // its ends and every 64 KB between, the size Windows hands memory out in: every page of it
    // was over a thousand reads, a few ms a query.
    const STEP: usize = 0x10000;
    let ok = if len <= STEP {
        probe(addr, len)
    } else {
        let end = addr.saturating_add(len);
        (addr..end - 8).step_by(STEP).all(|at| probe(at, 8)) && probe(end - 8, 8)
    };
    if !ok {
        FAILED.fetch_add(1, Relaxed);
        FAILED_NS.fetch_add(t.elapsed().as_nanos() as u64, Relaxed);
    }
    ok
}

pub fn read_u64(addr: usize) -> Option<u64> {
    readable(addr, 8).then(|| unsafe { *(addr as *const u64) })
}

/// Class name of the object at `addr` (its first qword must be a vtable with RTTI).
pub fn class_of(addr: usize) -> Option<String> {
    let vt = read_u64(addr)? as usize;
    if !readable(vt.wrapping_sub(8), 16) {
        return None;
    }
    unsafe { UnknownPtr::from(addr) }.rtti_classname()
}
