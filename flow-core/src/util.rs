use std::thread;

/// A wrapper type that aligns data to a cache line boundary (64 bytes).
/// This prevents false sharing between adjacent elements in arrays.
#[repr(align(64))]
#[derive(Clone, Debug, Default)]
pub struct CachelineAligned<T> {
    pub data: T,
}

impl<T> CachelineAligned<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T> std::ops::Deref for CachelineAligned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> std::ops::DerefMut for CachelineAligned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub(crate) struct WaitBackoff {
    spins: u32,
    yields: u32,
}

impl WaitBackoff {
    const MAX_SPINS: u32 = 64;
    const MAX_YIELDS: u32 = 32;

    pub(crate) fn new() -> Self {
        Self {
            spins: 0,
            yields: 0,
        }
    }

    pub(crate) fn try_snooze(&mut self) -> bool {
        if self.spins < Self::MAX_SPINS {
            self.spins += 1;
            std::hint::spin_loop();
            return true;
        }

        if self.yields < Self::MAX_YIELDS {
            self.yields += 1;
            thread::yield_now();
            return true;
        }

        false
    }
}
