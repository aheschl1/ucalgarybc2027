//! Per-bot memory accounting through CPython's allocator hooks. The hooks are
//! process-global, so each bot thread carries its own budget in a thread-local and a
//! thread without one passes through untouched. Two hooks see every byte pymalloc
//! hands out: arenas hold the small objects, the raw domain holds everything larger.
//! Raw blocks are sized with `malloc_usable_size`, which is why only Python's default
//! allocator is supported.

#![allow(unsafe_code)]

use std::cell::Cell;
use std::ffi::c_void;
use std::sync::OnceLock;

use pyo3::ffi;

#[derive(Clone, Copy)]
struct Budget {
    limit: u64,
    used: u64,
}

impl Budget {
    fn charge(&mut self, bytes: u64) -> bool {
        let used = self.used.saturating_add(bytes);
        if used > self.limit {
            return false;
        }
        self.used = used;
        true
    }

    fn release(&mut self, bytes: u64) {
        self.used = self.used.saturating_sub(bytes);
    }

    /// Replaces a provisional charge with what a block actually turned out to cost.
    fn settle(&mut self, charged: u64, old: u64, new: u64) {
        self.used = self.used.saturating_add(new).saturating_sub(charged + old);
    }
}

thread_local! {
    static BUDGET: Cell<Option<Budget>> = const { Cell::new(None) };
}

/// Starts charging this thread's allocations against `limit` bytes.
pub(super) fn enter(limit: u64) {
    BUDGET.set(Some(Budget { limit, used: 0 }));
}

pub(super) fn leave() {
    BUDGET.set(None);
}

/// Bytes this thread's budget holds, if it has one.
pub(super) fn used() -> Option<u64> {
    BUDGET.get().map(|b| b.used)
}

/// Runs `f` on this thread's budget. `None` when the thread has none, or during its
/// teardown.
fn with_budget<T>(f: impl FnOnce(&mut Budget) -> T) -> Option<T> {
    BUDGET
        .try_with(|cell| {
            let mut budget = cell.get()?;
            let out = f(&mut budget);
            cell.set(Some(budget));
            Some(out)
        })
        .ok()
        .flatten()
}

fn charge(bytes: u64) -> bool {
    with_budget(|b| b.charge(bytes)).unwrap_or(true)
}

fn release(bytes: u64) {
    with_budget(|b| b.release(bytes));
}

fn settle(charged: u64, old: u64, new: u64) {
    with_budget(|b| b.settle(charged, old, new));
}

/// The allocators that were in place before ours; ours delegate to them.
struct Original {
    raw: ffi::PyMemAllocatorEx,
    arena: ffi::PyObjectArenaAllocator,
}

// The contexts are CPython's own statics, read-only through these tables.
unsafe impl Sync for Original {}
unsafe impl Send for Original {}

static ORIGINAL: OnceLock<Original> = OnceLock::new();

fn original() -> &'static Original {
    ORIGINAL.get().expect("allocator hooks installed")
}

/// Wraps the allocators, once per process. Must hold the main GIL. Refuses any raw
/// allocator but the default, whose blocks `malloc_usable_size` could not size.
pub(super) fn install() -> Result<(), &'static str> {
    if ORIGINAL.get().is_some() {
        return Ok(());
    }
    unsafe {
        let mut raw = std::mem::zeroed::<ffi::PyMemAllocatorEx>();
        ffi::PyMem_GetAllocator(ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_RAW, &mut raw);
        if !raw.ctx.is_null() {
            return Err(
                "bot memory limits need Python's default allocator (unset PYTHONMALLOC, no -X dev)",
            );
        }
        let mut arena = std::mem::zeroed::<ffi::PyObjectArenaAllocator>();
        ffi::PyObject_GetArenaAllocator(&mut arena);
        let _ = ORIGINAL.set(Original { raw, arena });
        let mut hooked = ffi::PyMemAllocatorEx {
            ctx: std::ptr::null_mut(),
            malloc: Some(raw_malloc),
            calloc: Some(raw_calloc),
            realloc: Some(raw_realloc),
            free: Some(raw_free),
        };
        ffi::PyMem_SetAllocator(ffi::PyMemAllocatorDomain::PYMEM_DOMAIN_RAW, &mut hooked);
        let mut hooked = ffi::PyObjectArenaAllocator {
            ctx: std::ptr::null_mut(),
            alloc: Some(arena_alloc),
            free: Some(arena_free),
        };
        ffi::PyObject_SetArenaAllocator(&mut hooked);
    }
    Ok(())
}

fn usable(ptr: *mut c_void) -> u64 {
    if ptr.is_null() {
        0
    } else {
        unsafe { libc::malloc_usable_size(ptr) as u64 }
    }
}

extern "C" fn arena_alloc(_ctx: *mut c_void, size: usize) -> *mut c_void {
    if !charge(size as u64) {
        return std::ptr::null_mut();
    }
    let arena = &original().arena;
    let ptr = (arena.alloc.expect("arena alloc"))(arena.ctx, size);
    if ptr.is_null() {
        release(size as u64);
    }
    ptr
}

extern "C" fn arena_free(_ctx: *mut c_void, ptr: *mut c_void, size: usize) {
    release(size as u64);
    let arena = &original().arena;
    (arena.free.expect("arena free"))(arena.ctx, ptr, size);
}

extern "C" fn raw_malloc(_ctx: *mut c_void, size: usize) -> *mut c_void {
    if !charge(size as u64) {
        return std::ptr::null_mut();
    }
    let raw = &original().raw;
    let ptr = (raw.malloc.expect("raw malloc"))(raw.ctx, size);
    settle(size as u64, 0, usable(ptr));
    ptr
}

extern "C" fn raw_calloc(_ctx: *mut c_void, nelem: usize, elsize: usize) -> *mut c_void {
    let size = nelem.saturating_mul(elsize) as u64;
    if !charge(size) {
        return std::ptr::null_mut();
    }
    let raw = &original().raw;
    let ptr = (raw.calloc.expect("raw calloc"))(raw.ctx, nelem, elsize);
    settle(size, 0, usable(ptr));
    ptr
}

extern "C" fn raw_realloc(_ctx: *mut c_void, ptr: *mut c_void, size: usize) -> *mut c_void {
    let old = usable(ptr);
    let growth = (size as u64).saturating_sub(old);
    if !charge(growth) {
        return std::ptr::null_mut();
    }
    let raw = &original().raw;
    let moved = (raw.realloc.expect("raw realloc"))(raw.ctx, ptr, size);
    if moved.is_null() {
        release(growth);
        return moved;
    }
    settle(growth, old, usable(moved));
    moved
}

extern "C" fn raw_free(_ctx: *mut c_void, ptr: *mut c_void) {
    release(usable(ptr));
    let raw = &original().raw;
    (raw.free.expect("raw free"))(raw.ctx, ptr);
}
