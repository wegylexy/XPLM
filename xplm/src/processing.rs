//! Flight loop callbacks (`XPLMProcessing.h`), the first RAII + trampoline
//! wrapper — the pattern established here (boxed closure behind a raw
//! `refcon` pointer, a single panic-guarded `extern "C"` trampoline, `Drop`
//! unregistering the native callback) repeats for menus, windows, and
//! instances in later phases.

use std::ffi::c_void;
use std::os::raw::{c_float, c_int};

use xplm_sys::{
    xplm_FlightLoop_Phase_AfterFlightModel, xplm_FlightLoop_Phase_BeforeFlightModel,
    XPLMCreateFlightLoop, XPLMCreateFlightLoop_t, XPLMDestroyFlightLoop, XPLMFlightLoopID,
    XPLMGetCycleNumber, XPLMGetElapsedTime, XPLMScheduleFlightLoop,
};

/// Seconds since the sim started, per `XPLMGetElapsedTime` — not wall-clock
/// time, and not paused-aware; see the SDK's own caveats about drift versus
/// a flight loop callback's own elapsed-time parameters.
pub fn elapsed_time() -> f32 {
    unsafe { XPLMGetElapsedTime() }
}

/// A counter incrementing once per sim cycle computed/frame rendered, per
/// `XPLMGetCycleNumber`.
pub fn cycle_number() -> i32 {
    unsafe { XPLMGetCycleNumber() }
}

/// The callback signature X-Plane invokes each flight loop dispatch:
/// `(elapsed_since_last_call, elapsed_since_last_flight_loop, counter) -> next_interval`.
/// See `XPLMFlightLoop_f` in `XPLMProcessing.h` for the return-value contract
/// (0 = stop, positive = seconds, negative = frames).
type Callback = dyn FnMut(f32, f32, i32) -> f32 + 'static;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightLoopPhase {
    BeforeFlightModel,
    AfterFlightModel,
}

impl From<FlightLoopPhase> for xplm_sys::XPLMFlightLoopPhaseType {
    fn from(phase: FlightLoopPhase) -> Self {
        match phase {
            FlightLoopPhase::BeforeFlightModel => xplm_FlightLoop_Phase_BeforeFlightModel,
            FlightLoopPhase::AfterFlightModel => xplm_FlightLoop_Phase_AfterFlightModel,
        }
    }
}

/// A registered flight loop callback. Dropping it unregisters the callback
/// from X-Plane and frees the boxed closure — no manual unregister call
/// needed, unlike the C# original's explicit `Dispose()`.
pub struct FlightLoop {
    id: XPLMFlightLoopID,
    // A thin pointer to a heap-allocated fat pointer (`Box<Callback>` is
    // itself a (data, vtable) pair and can't be cast to `*mut c_void`
    // directly) — the extra indirection is what lets the trait-object
    // closure round-trip through X-Plane's `void *` refcon.
    refcon: *mut Box<Callback>,
}

// `*mut Box<Callback>` makes this !Send/!Sync by default. X-Plane only ever
// calls flight loop callbacks (and Drop, if it runs off the same thread)
// from its single main thread, so there's never concurrent access to the
// boxed closure — but plugin state holding a FlightLoop still needs to live
// in a `static Mutex<Option<_>>` (see `register_plugin!`), which requires
// Send. Not Sync: nothing here supports being read from two threads at once.
unsafe impl Send for FlightLoop {}

impl FlightLoop {
    pub fn new(
        phase: FlightLoopPhase,
        callback: impl FnMut(f32, f32, i32) -> f32 + 'static,
    ) -> Self {
        let boxed: Box<Callback> = Box::new(callback);
        let refcon = Box::into_raw(Box::new(boxed));

        let mut params = XPLMCreateFlightLoop_t {
            structSize: std::mem::size_of::<XPLMCreateFlightLoop_t>() as c_int,
            phase: phase.into(),
            callbackFunc: Some(trampoline),
            refcon: refcon as *mut c_void,
        };
        let id = unsafe { XPLMCreateFlightLoop(&mut params) };

        Self { id, refcon }
    }

    /// Schedules (or reschedules) this callback. Positive `interval` is
    /// seconds, negative is a frame count, 0 deactivates it without
    /// unregistering. See `XPLMScheduleFlightLoop`.
    pub fn schedule(&self, interval: f32, relative_to_now: bool) {
        unsafe {
            XPLMScheduleFlightLoop(self.id, interval, relative_to_now as c_int);
        }
    }
}

impl Drop for FlightLoop {
    fn drop(&mut self) {
        unsafe {
            XPLMDestroyFlightLoop(self.id);
            drop(Box::from_raw(self.refcon));
        }
    }
}

unsafe extern "C" fn trampoline(
    elapsed_since_last_call: c_float,
    elapsed_time_since_last_flight_loop: c_float,
    counter: c_int,
    refcon: *mut c_void,
) -> c_float {
    crate::guard(|| {
        let callback: &mut Callback = unsafe { &mut *(*(refcon as *mut Box<Callback>)) };
        callback(
            elapsed_since_last_call,
            elapsed_time_since_last_flight_loop,
            counter,
        )
    })
    .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// Exercises the trampoline directly, without XPLMCreateFlightLoop —
    /// that call requires XPLM_64.dll to be loaded by an actual X-Plane
    /// host, which a plain `cargo test` process doesn't provide. This still
    /// proves the closure-registry half of the pattern: refcon round-trips
    /// through the trampoline to the right closure instance.
    #[test]
    fn trampoline_invokes_boxed_closure() {
        let calls: Rc<Cell<i32>> = Rc::new(Cell::new(0));
        let calls_clone = calls.clone();
        let boxed: Box<Callback> = Box::new(move |elapsed, _since_loop, counter| {
            calls_clone.set(calls_clone.get() + 1);
            elapsed + counter as f32
        });
        let refcon = Box::into_raw(Box::new(boxed));

        let result = unsafe { trampoline(1.5, 0.0, 3, refcon as *mut c_void) };

        assert_eq!(result, 4.5);
        assert_eq!(calls.get(), 1);

        unsafe { drop(Box::from_raw(refcon)) };
    }

    #[test]
    fn trampoline_catches_panic_and_returns_zero() {
        let boxed: Box<Callback> = Box::new(|_, _, _| panic!("boom"));
        let refcon = Box::into_raw(Box::new(boxed));

        let result = unsafe { trampoline(0.0, 0.0, 0, refcon as *mut c_void) };

        assert_eq!(result, 0.0);

        unsafe { drop(Box::from_raw(refcon)) };
    }
}
