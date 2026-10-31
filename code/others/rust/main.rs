use std::sync::atomic::{AtomicUsize, Ordering};
use std::marker::PhantomData;
use std::ptr;

/// A lock-free, single-producer single-consumer bounded ring buffer stack.
/// Showcases atomic ordering variance, raw pointer arithmetic, and explicit memory boundaries.
pub struct AdvancedLockFreeRing<T, const N: usize> {
    buffer: *mut T,
    head: AtomicUsize,
    tail: AtomicUsize,
    _marker: PhantomData<T>, // Informs compiler of variance and dropped bounds for type T
}

impl<T, const N: usize> AdvancedLockFreeRing<T, N> {
    pub fn new() -> Self {
        // Assert at compile-time/const context that capacity is a power of 2 for fast masking
        assert!(N.is_power_of_two(), "Capacity N must be a power of two");
        
        let mut layout_alloc = Vec::with_capacity(N);
        let buffer = layout_alloc.as_mut_ptr();
        std::mem::forget(layout_alloc); // Manually bypass standard RAII drop systems

        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            _marker: PhantomData,
        }
    }

    /// Pushes an item to the ring buffer. 
    /// Employs Release ordering to guarantee memory visibility to the reading thread.
    pub fn push(&self, value: T) -> Result<(), T> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        if head.wrapping_sub(tail) == N {
            return Err(value); // Ring buffer structural saturation boundary met
        }

        // Unsafe block for direct raw pointer offset arithmetic and uninitialised memory writes
        unsafe {
            let offset = (head & (N - 1)) as isize;
            let target_ptr = self.buffer.offset(offset);
            ptr::write(target_ptr, value);
        }

        // Standard release barrier synchronises with subsequent Acquire operations
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Pops an item off the buffer queue pipeline.
    /// Uses Acquire ordering to ensure memory operations aren't reordered before this step.
    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail == head {
            return None; // Buffer depletion boundary reached
        }

        let value = unsafe {
            let offset = (tail & (N - 1)) as isize;
            let target_ptr = self.buffer.offset(offset);
            ptr::read(target_ptr) // Safely extract value from unmanaged memory block
        };

        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(value)
    }
}

// Explicitly implement Drop to prevent memory leaks from forgotten vectors
impl<T, const N: usize> Drop for AdvancedLockFreeRing<T, N> {
    fn drop(&mut self) {
        let mut tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);

        while tail != head {
            unsafe {
                let offset = (tail & (N - 1)) as isize;
                ptr::drop_in_place(self.buffer.offset(offset));
            }
            tail = tail.wrapping_add(1);
        }
        // Re-constitute vector allocations back into standard memory management
        unsafe {
            let _ = Vec::from_raw_parts(self.buffer, 0, N);
        }
    }
}

// Safely bridge threads across parallel pipelines
unsafe impl<T: Send, const N: usize> Send for AdvancedLockFreeRing<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for AdvancedLockFreeRing<T, N> {}

fn main() {
    let pipeline_ring = AdvancedLockFreeRing::<String, 4>::new();
    
    pipeline_ring.push("KernelData_0x01".to_string()).unwrap();
    pipeline_ring.push("KernelData_0x02".to_string()).unwrap();

    if let Some(data) = pipeline_ring.pop() {
        println!("[Pipeline Stream Out]: {}", data);
    }
}
