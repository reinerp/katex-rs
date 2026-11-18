//! A bump allocator for the single-threaded wasm module: `free` does nothing, and the memory of a render is taken back all at once
//! (`reset`) when the render is done. Two regions: the persistent one (the `KatexContext`, the settings, the output buffer: whatever is
//! allocated while `persistent` is on) and the one that `reset` empties (everything a render allocates).
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

const PAGE: usize = 65_536;
const CHUNKS: usize = 48;

#[derive(Clone, Copy)]
struct Region {
    /// (start, end) of the chunks taken from linear memory, in the order they were taken
    chunks: [(usize, usize); CHUNKS],
    n: usize,
    /// the chunk allocations come from, and where in it
    cur: usize,
    ptr: usize,
    /// the chunk size to ask for at least
    min_chunk: usize,
}

impl Region {
    const fn new(min_chunk: usize) -> Self {
        Self { chunks: [(0, 0); CHUNKS], n: 0, cur: 0, ptr: 0, min_chunk }
    }

    /// Takes a chunk of at least `need` bytes from the end of linear memory.
    fn grow(&mut self, need: usize) -> bool {
        if self.n == CHUNKS {
            return false;
        }
        let pages = (need.max(self.min_chunk) + PAGE - 1) / PAGE;
        let old = core::arch::wasm32::memory_grow(0, pages);
        if old == usize::MAX {
            return false;
        }
        self.chunks[self.n] = (old * PAGE, (old + pages) * PAGE);
        self.n += 1;
        true
    }

    fn alloc(&mut self, size: usize, align: usize) -> *mut u8 {
        loop {
            if self.n > 0 {
                let p = (self.ptr + align - 1) & !(align - 1);
                if p + size <= self.chunks[self.cur].1 {
                    self.ptr = p + size;
                    return p as *mut u8;
                }
                // the next chunk we already own (after a reset), if it is big enough
                if self.cur + 1 < self.n {
                    self.cur += 1;
                    self.ptr = self.chunks[self.cur].0;
                    continue;
                }
            }
            if !self.grow(size + align) {
                return core::ptr::null_mut();
            }
            self.cur = self.n - 1;
            self.ptr = self.chunks[self.cur].0;
        }
    }

    fn reset(&mut self) {
        self.cur = 0;
        self.ptr = if self.n > 0 { self.chunks[0].0 } else { 0 };
    }

    fn owns_end(&self, p: usize, size: usize) -> bool {
        self.n > 0 && p + size == self.ptr && p >= self.chunks[self.cur].0
    }
}

struct Inner {
    persistent: Region,
    scratch: Region,
    persistent_on: bool,
}

pub struct Bump(UnsafeCell<Inner>);

// SAFETY: the module is single-threaded (wasm32-unknown-unknown without the threads feature).
unsafe impl Sync for Bump {}

impl Bump {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(Inner { persistent: Region::new(PAGE), scratch: Region::new(PAGE * 16), persistent_on: false }))
    }

    #[allow(clippy::mut_from_ref)]
    fn inner(&self) -> &mut Inner {
        // SAFETY: single-threaded, and no reference is kept across a call into the allocator.
        unsafe { &mut *self.0.get() }
    }

    /// Whether the allocations that follow are persistent (until set to false) rather than taken back by `reset`.
    pub fn set_persistent(&self, on: bool) -> bool {
        core::mem::replace(&mut self.inner().persistent_on, on)
    }

    /// Takes back everything allocated since the start, or since the last reset, outside the persistent region.
    pub fn reset(&self) {
        self.inner().scratch.reset();
    }
}

unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let i = self.inner();
        let r = if i.persistent_on { &mut i.persistent } else { &mut i.scratch };
        r.alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let i = self.inner();
        let p = ptr as usize;
        // grow in place when the block is the last one of its region and the chunk has the room
        for r in [&mut i.scratch, &mut i.persistent] {
            if r.owns_end(p, layout.size()) {
                if p + new_size <= r.chunks[r.cur].1 {
                    r.ptr = p + new_size;
                    return ptr;
                }
                break;
            }
        }
        let r = if i.persistent_on { &mut i.persistent } else { &mut i.scratch };
        let q = r.alloc(new_size, layout.align());
        if !q.is_null() {
            unsafe { core::ptr::copy_nonoverlapping(ptr, q, layout.size().min(new_size)) };
        }
        q
    }
}
