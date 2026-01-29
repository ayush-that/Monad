//! Lock-free buffer implementations for real-time audio.

pub mod ring;

pub use ring::{shared_ring_buffer, RingBuffer, SharedRingBuffer};
