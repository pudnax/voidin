use std::{
    ops::Range,
    path::{Path, PathBuf},
};

pub fn assets_dir() -> PathBuf {
    Path::new(env!("OUT_DIR")).join("assets")
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
