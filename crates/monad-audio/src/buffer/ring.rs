//! Lock-free ring buffer for audio streaming.
//!
//! This buffer is designed for single-producer, single-consumer scenarios
//! where a decode thread writes samples and an audio callback reads them.

#![allow(clippy::unwrap_used)] // Tests use unwrap for brevity

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Lock-free single-producer, single-consumer ring buffer.
///
/// Designed for real-time audio where allocations in the hot path are forbidden.
/// Uses atomic operations for thread-safe read/write without locks.
pub struct RingBuffer {
    /// The underlying buffer storage.
    buffer: Box<[f32]>,
    /// Current read position.
    read_pos: AtomicUsize,
    /// Current write position.
    write_pos: AtomicUsize,
    /// Buffer capacity (power of 2 for efficient modulo).
    capacity: usize,
    /// Mask for efficient modulo (capacity - 1).
    mask: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the specified capacity.
    ///
    /// The capacity will be rounded up to the next power of 2.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        let buffer = vec![0.0f32; capacity].into_boxed_slice();

        Self {
            buffer,
            read_pos: AtomicUsize::new(0),
            write_pos: AtomicUsize::new(0),
            capacity,
            mask: capacity - 1,
        }
    }

    /// Get the buffer capacity.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the number of samples available for reading.
    pub fn available(&self) -> usize {
        let write = self.write_pos.load(Ordering::Acquire);
        let read = self.read_pos.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Get the number of free slots for writing.
    pub fn free(&self) -> usize {
        self.capacity - self.available()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.available() == 0
    }

    /// Check if the buffer is full.
    pub fn is_full(&self) -> bool {
        self.free() == 0
    }

    /// Write samples to the buffer.
    ///
    /// Returns the number of samples actually written.
    /// This method is designed to be called from the producer thread.
    pub fn write(&self, samples: &[f32]) -> usize {
        let write_pos = self.write_pos.load(Ordering::Relaxed);
        let read_pos = self.read_pos.load(Ordering::Acquire);

        let available_space = self.capacity - write_pos.wrapping_sub(read_pos);
        let to_write = samples.len().min(available_space);

        if to_write == 0 {
            return 0;
        }

        let start_idx = write_pos & self.mask;
        let end_idx = (write_pos + to_write) & self.mask;

        // SAFETY: We're the only writer and we've checked bounds
        let buffer_ptr = self.buffer.as_ptr().cast_mut();

        #[allow(unsafe_code)]
        if start_idx < end_idx || to_write <= self.capacity - start_idx {
            // Contiguous write
            // SAFETY: We're the only writer and indices are within bounds
            unsafe {
                std::ptr::copy_nonoverlapping(
                    samples.as_ptr(),
                    buffer_ptr.add(start_idx),
                    to_write,
                );
            }
        } else {
            // Wrap-around write
            let first_chunk = self.capacity - start_idx;
            // SAFETY: We're the only writer and indices are within bounds
            unsafe {
                std::ptr::copy_nonoverlapping(
                    samples.as_ptr(),
                    buffer_ptr.add(start_idx),
                    first_chunk,
                );
                std::ptr::copy_nonoverlapping(
                    samples.as_ptr().add(first_chunk),
                    buffer_ptr,
                    to_write - first_chunk,
                );
            }
        }

        self.write_pos
            .store(write_pos.wrapping_add(to_write), Ordering::Release);

        to_write
    }

    /// Read samples from the buffer.
    ///
    /// Returns the number of samples actually read.
    /// This method is designed to be called from the consumer thread.
    pub fn read(&self, output: &mut [f32]) -> usize {
        let read_pos = self.read_pos.load(Ordering::Relaxed);
        let write_pos = self.write_pos.load(Ordering::Acquire);

        let available = write_pos.wrapping_sub(read_pos);
        let to_read = output.len().min(available);

        if to_read == 0 {
            return 0;
        }

        let start_idx = read_pos & self.mask;

        // SAFETY: We're the only reader and we've checked bounds
        let buffer_ptr = self.buffer.as_ptr();

        #[allow(unsafe_code)]
        if start_idx + to_read <= self.capacity {
            // Contiguous read
            // SAFETY: We're the only reader and indices are within bounds
            unsafe {
                std::ptr::copy_nonoverlapping(
                    buffer_ptr.add(start_idx),
                    output.as_mut_ptr(),
                    to_read,
                );
            }
        } else {
            // Wrap-around read
            let first_chunk = self.capacity - start_idx;
            // SAFETY: We're the only reader and indices are within bounds
            unsafe {
                std::ptr::copy_nonoverlapping(
                    buffer_ptr.add(start_idx),
                    output.as_mut_ptr(),
                    first_chunk,
                );
                std::ptr::copy_nonoverlapping(
                    buffer_ptr,
                    output.as_mut_ptr().add(first_chunk),
                    to_read - first_chunk,
                );
            }
        }

        self.read_pos
            .store(read_pos.wrapping_add(to_read), Ordering::Release);

        to_read
    }

    /// Read samples without advancing the read position (peek).
    pub fn peek(&self, output: &mut [f32]) -> usize {
        let read_pos = self.read_pos.load(Ordering::Relaxed);
        let write_pos = self.write_pos.load(Ordering::Acquire);

        let available = write_pos.wrapping_sub(read_pos);
        let to_read = output.len().min(available);

        if to_read == 0 {
            return 0;
        }

        let start_idx = read_pos & self.mask;
        let buffer_ptr = self.buffer.as_ptr();

        #[allow(unsafe_code)]
        if start_idx + to_read <= self.capacity {
            // SAFETY: We're the only reader and indices are within bounds
            unsafe {
                std::ptr::copy_nonoverlapping(
                    buffer_ptr.add(start_idx),
                    output.as_mut_ptr(),
                    to_read,
                );
            }
        } else {
            let first_chunk = self.capacity - start_idx;
            // SAFETY: We're the only reader and indices are within bounds
            unsafe {
                std::ptr::copy_nonoverlapping(
                    buffer_ptr.add(start_idx),
                    output.as_mut_ptr(),
                    first_chunk,
                );
                std::ptr::copy_nonoverlapping(
                    buffer_ptr,
                    output.as_mut_ptr().add(first_chunk),
                    to_read - first_chunk,
                );
            }
        }

        to_read
    }

    /// Skip samples without reading them.
    pub fn skip(&self, count: usize) -> usize {
        let read_pos = self.read_pos.load(Ordering::Relaxed);
        let write_pos = self.write_pos.load(Ordering::Acquire);

        let available = write_pos.wrapping_sub(read_pos);
        let to_skip = count.min(available);

        self.read_pos
            .store(read_pos.wrapping_add(to_skip), Ordering::Release);

        to_skip
    }

    /// Clear the buffer.
    pub fn clear(&self) {
        let write_pos = self.write_pos.load(Ordering::Relaxed);
        self.read_pos.store(write_pos, Ordering::Release);
    }
}

// SAFETY: RingBuffer is safe to share between threads (Send + Sync).
// The producer and consumer operate on different positions with atomic ordering.
// The buffer uses atomic operations to coordinate read/write positions,
// ensuring no data races occur between the single producer and single consumer.
#[allow(unsafe_code)]
unsafe impl Send for RingBuffer {}
#[allow(unsafe_code)]
unsafe impl Sync for RingBuffer {}

/// Thread-safe reference to a ring buffer.
pub type SharedRingBuffer = Arc<RingBuffer>;

/// Create a new shared ring buffer.
pub fn shared_ring_buffer(capacity: usize) -> SharedRingBuffer {
    Arc::new(RingBuffer::new(capacity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_basic_write_read() {
        let buffer = RingBuffer::new(1024);

        let samples = [1.0f32, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(buffer.write(&samples), 5);
        assert_eq!(buffer.available(), 5);

        let mut output = [0.0f32; 5];
        assert_eq!(buffer.read(&mut output), 5);
        assert_eq!(output, samples);
        assert_eq!(buffer.available(), 0);
    }

    #[test]
    fn test_wraparound() {
        let buffer = RingBuffer::new(8); // Will be 8 (already power of 2)

        // Fill most of the buffer
        let samples1 = [1.0f32; 6];
        assert_eq!(buffer.write(&samples1), 6);

        // Read some
        let mut output = [0.0f32; 4];
        assert_eq!(buffer.read(&mut output), 4);

        // Write more (should wrap around)
        let samples2 = [2.0f32; 5];
        assert_eq!(buffer.write(&samples2), 5);

        // Read everything
        let mut final_output = [0.0f32; 7];
        assert_eq!(buffer.read(&mut final_output), 7);
        assert_eq!(&final_output[0..2], &[1.0, 1.0]); // Remaining from samples1
        assert_eq!(&final_output[2..7], &[2.0; 5]); // All of samples2
    }

    #[test]
    fn test_full_buffer() {
        let buffer = RingBuffer::new(4);

        let samples = [1.0f32; 4];
        assert_eq!(buffer.write(&samples), 4);
        assert!(buffer.is_full());

        // Should not be able to write more
        assert_eq!(buffer.write(&[2.0]), 0);

        // Read one, then we can write one
        let mut output = [0.0f32; 1];
        buffer.read(&mut output);
        assert_eq!(buffer.write(&[2.0]), 1);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_peek() {
        let buffer = RingBuffer::new(16);

        let samples = [1.0f32, 2.0, 3.0];
        buffer.write(&samples);

        let mut peeked = [0.0f32; 3];
        assert_eq!(buffer.peek(&mut peeked), 3);
        assert_eq!(peeked, samples);

        // Available should not have changed
        assert_eq!(buffer.available(), 3);

        // Can still read the same data
        let mut output = [0.0f32; 3];
        assert_eq!(buffer.read(&mut output), 3);
        assert_eq!(output, samples);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_skip() {
        let buffer = RingBuffer::new(16);

        let samples = [1.0f32, 2.0, 3.0, 4.0, 5.0];
        buffer.write(&samples);

        assert_eq!(buffer.skip(2), 2);
        assert_eq!(buffer.available(), 3);

        let mut output = [0.0f32; 3];
        buffer.read(&mut output);
        assert_eq!(output, [3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_clear() {
        let buffer = RingBuffer::new(16);

        let samples = [1.0f32; 10];
        buffer.write(&samples);
        assert_eq!(buffer.available(), 10);

        buffer.clear();
        assert_eq!(buffer.available(), 0);
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;

        let buffer = Arc::new(RingBuffer::new(1024));
        let buffer_writer = buffer.clone();
        let buffer_reader = buffer;

        let writer = thread::spawn(move || {
            let samples = [1.0f32; 100];
            let mut total_written = 0;
            while total_written < 10000 {
                let written = buffer_writer.write(&samples);
                total_written += written;
                if written == 0 {
                    thread::yield_now();
                }
            }
            total_written
        });

        let reader = thread::spawn(move || {
            let mut output = [0.0f32; 100];
            let mut total_read = 0;
            while total_read < 10000 {
                let read = buffer_reader.read(&mut output);
                total_read += read;
                if read == 0 {
                    thread::yield_now();
                }
            }
            total_read
        });

        let written = writer.join().unwrap();
        let read = reader.join().unwrap();

        assert!(written >= 10000);
        assert!(read >= 10000);
    }
}
