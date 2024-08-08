#![doc = include_str!("../../README.md")]

pub use dynosaur_derive::*;

pub mod macro_lib {
    pub struct Ref<T> {
        t: T,
    }

    impl<T> Ref<T> {
        pub unsafe fn new(t: T) -> Self {
            Self { t }
        }
    }

    impl<T> std::ops::Deref for Ref<T> {
        type Target = T;

        fn deref(&self) -> &T {
            &self.t
        }
    }

    /// Newtype that permits borrowed (`&mut T`) or shared (`&T`) access,
    /// but nothing else.
    pub struct RefMut<T> {
        t: T,
    }

    impl<T> RefMut<T> {
        pub fn new(t: T) -> Self {
            Self { t }
        }
    }

    impl<T> std::ops::Deref for RefMut<T> {
        type Target = T;

        fn deref(&self) -> &T {
            &self.t
        }
    }

    impl<T> std::ops::DerefMut for RefMut<T> {
        fn deref_mut(&mut self) -> &mut T {
            &mut self.t
        }
    }
}
