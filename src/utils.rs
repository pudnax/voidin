use std::{mem::size_of, num::NonZeroU64, ops::Range, path::PathBuf};

pub trait NonZeroSized: Sized {
    const NSIZE: NonZeroU64 = unsafe { NonZeroU64::new_unchecked(size_of::<Self>() as _) };
}
impl<T> NonZeroSized for T where T: Sized {}

pub fn assets_dir() -> PathBuf {
    // Path::new(env!("OUT_DIR")).join("assets")
    PathBuf::new().join("assets")
}

pub trait Lerp: Sized {
    fn lerp(self, range: Range<Self>) -> Self;
}

impl Lerp for f32 {
    fn lerp(self, Range { start: a, end: b }: Range<Self>) -> Self {
        a * (1. - self) + b * self
    }
}

impl Lerp for f64 {
    fn lerp(self, Range { start: a, end: b }: Range<Self>) -> Self {
        a * (1. - self) + b * self
    }
}
