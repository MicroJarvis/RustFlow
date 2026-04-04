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