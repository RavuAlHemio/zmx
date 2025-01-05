//! CriticalSection-based mutex.
//!
//! Rust has moved to SRWLock for mutexes; those are only supported since Windows Vista and we're
//! trying to be compatible with some, uh, embarrassingly ancient Windows versions.

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

use windows::Win32::System::Threading::{
    EnterCriticalSection, InitializeCriticalSection, LeaveCriticalSection, CRITICAL_SECTION,
};


pub struct Mutex<T: ?Sized> {
    critical_section_cell: UnsafeCell<CRITICAL_SECTION>,
    data_cell: UnsafeCell<T>,
}
impl<T> Mutex<T> {
    pub fn new(data: T) -> Self {
        let mut critical_section = CRITICAL_SECTION::default();
        unsafe { InitializeCriticalSection(&mut critical_section) };

        let critical_section_cell = UnsafeCell::new(critical_section);
        let data_cell = UnsafeCell::new(data);

        Self {
            critical_section_cell,
            data_cell,
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        unsafe {
            EnterCriticalSection(self.critical_section_cell.get());
            MutexGuard::new(self)
        }
    }

    #[allow(unused)]
    pub fn unlock(guard: MutexGuard<'_, T>) {
        drop(guard)
    }
}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Sync> Sync for Mutex<T> {}

pub struct MutexGuard<'mutex, T: ?Sized + 'mutex> {
    lock: &'mutex Mutex<T>,
}
impl<'mutex, T> MutexGuard<'mutex, T> {
    unsafe fn new(lock: &'mutex Mutex<T>) -> Self {
        Self {
            lock,
        }
    }
}
impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data_cell.get() }
    }
}
impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data_cell.get() }
    }
}
impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { LeaveCriticalSection(self.lock.critical_section_cell.get()) };
    }
}
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}
