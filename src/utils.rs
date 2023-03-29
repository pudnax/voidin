use std::{
    iter::{self, Repeat},
    mem::size_of,
    num::NonZeroU64,
    ops::Range,
    time::Duration,
};

use itertools::Either;
use wgpu_profiler::GpuTimerScopeResult;

pub trait NonZeroSized: Sized {
    const NSIZE: NonZeroU64 = unsafe { NonZeroU64::new_unchecked(size_of::<Self>() as _) };
}
impl<T> NonZeroSized for T where T: Sized {}

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

pub trait UnwrapRepeat<T: Default + Clone, I>
where
    I: Iterator<Item = T>,
{
    fn unwrap_repeat(self) -> Either<I, Repeat<T>>;
}

impl<T: Default + Clone, I> UnwrapRepeat<T, I> for Option<I>
where
    I: Iterator<Item = T>,
{
    fn unwrap_repeat(self) -> Either<I, Repeat<T>> {
        match self {
            Some(iter) => Either::Left(iter),
            None => Either::Right(iter::repeat(T::default())),
        }
    }
}

pub fn scopes_to_console_recursive(results: &[GpuTimerScopeResult], indentation: usize) {
    for scope in results {
        if indentation > 0 {
            print!("{:<width$}", "|", width = 4 * indentation);
        }
        println!(
            "{:?} - {}",
            Duration::from_micros(((scope.time.end - scope.time.start) * 1e6) as u64),
            scope.label
        );
        if !scope.nested_scopes.is_empty() {
            scopes_to_console_recursive(&scope.nested_scopes, indentation + 1);
        }
    }
}
