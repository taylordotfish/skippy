#[cfg(feature = "std")]
use core::cell::Cell;

#[cfg(not(feature = "std"))]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "std")]
thread_local! {
    static CAN_SAFELY_DESTROY: Cell<bool> = Cell::new(true);
}

#[cfg(not(feature = "std"))]
static CAN_SAFELY_DESTROY: AtomicBool = AtomicBool::new(true);

pub fn can_safely_destroy() -> bool {
    #[cfg(feature = "std")]
    return CAN_SAFELY_DESTROY.with(Cell::get);
    #[cfg(not(feature = "std"))]
    return CAN_SAFELY_DESTROY.load(Ordering::Relaxed);
}

pub fn set_cannot_safely_destroy() {
    #[cfg(feature = "std")]
    CAN_SAFELY_DESTROY.with(|c| c.set(false));
    #[cfg(not(feature = "std"))]
    CAN_SAFELY_DESTROY.store(false, Ordering::Relaxed);
}

pub struct SetUnsafeOnDrop;

impl Drop for SetUnsafeOnDrop {
    fn drop(&mut self) {
        set_cannot_safely_destroy();
    }
}
