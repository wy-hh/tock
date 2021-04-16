//! Hardware agnostic interfaces for time and timers within the Tock
//! kernel.
//!
//! These traits are designed to be able encompass the wide
//! variety of hardare counters in a general yet efficient way. They
//! abstract the frequency of a counter through the `Frequency` trait
//! and the width of a time value through the `Ticks`
//! trait. Higher-level software abstractions should generally rely on
//! standard and common implementations of these traits (e.g.. `u32`
//! ticks and 16MHz frequency).  Hardware counter implementations and
//! peripherals can represent the actual hardware units an translate
//! into these more general ones.

use crate::ErrorCode;
use core::cmp::{Eq, Ord, Ordering, PartialOrd};
use core::fmt;

/// An integer type defining the width of a time value, which allows
/// clients to know when wraparound will occur.

pub trait Ticks: Clone + Copy + From<u32> + fmt::Debug + Ord + PartialOrd + Eq {
    /// Converts the type into a `usize`, stripping the higher bits
    /// it if it is larger than `usize` and filling the higher bits
    /// with 0 if it is smaller than `usize`.
    fn into_usize(self) -> usize;

    /// Converts the type into a `u32`, stripping the higher bits
    /// it if it is larger than `u32` and filling the higher bits
    /// with 0 if it is smaller than `u32`. Included as a simple
    /// helper since Tock uses `u32` pervasively and most platforms
    /// are 32 bits.
    fn into_u32(self) -> u32;

    /// Add two values, wrapping around on overflow using standard
    /// unsigned arithmetic.
    fn wrapping_add(self, other: Self) -> Self;
    /// Subtract two values, wrapping around on underflow using standard
    /// unsigned arithmetic.
    fn wrapping_sub(self, other: Self) -> Self;

    /// Returns whether the value is in the range of [`start, `end`) using
    /// unsigned arithmetic and considering wraparound. It returns `true`
    /// if, incrementing from `start`, the value will be reached before `end`.
    /// Put another way, it returns `(self - start) < (end - start)` in
    /// unsigned arithmetic.
    fn within_range(self, start: Self, end: Self) -> bool;

    /// Returns the maximum value of this type, which should be (2^width)-1.
    fn max_value() -> Self;
}

/// Represents a clock's frequency in Hz, allowing code to transform
/// between computer time units and wall clock time. It is typically
/// an associated type for an implementation of the `Time` trait.
pub trait Frequency {
    /// Returns frequency in Hz.
    fn frequency() -> u32;
}

/// Represents a moment in time, obtained by calling `now`.
pub trait Time {
    /// The number of ticks per second
    type Frequency: Frequency;
    /// The width of a time value
    type Ticks: Ticks;

    /// Returns a timestamp. Depending on the implementation of
    /// Time, this could represent either a static timestamp or
    /// a sample of a counter; if an implementation relies on
    /// it being constant or changing it should use `Timestamp`
    /// or `Counter`.
    fn now(&self) -> Self::Ticks;

    /// Returns the number of ticks in the provided number of seconds,
    /// rounding down any fractions. If the value overflows Ticks it
    /// returns `Ticks::max_value()`.
    fn ticks_from_seconds(s: u32) -> Self::Ticks {
        let val: u64 = Self::Frequency::frequency() as u64 * s as u64;
        ticks_from_val(val)
    }

    /// Returns the number of ticks in the provided number of milliseconds,
    /// rounding down any fractions. If the value overflows Ticks it
    /// returns `Ticks::max_value()`.
    fn ticks_from_ms(ms: u32) -> Self::Ticks {
        let val: u64 = Self::Frequency::frequency() as u64 * ms as u64;
        ticks_from_val(val / 1000)
    }

    /// Returns the number of ticks in the provided number of microseconds,
    /// rounding down any fractions. If the value overflows Ticks it
    /// returns `Ticks::max_value()`.
    fn ticks_from_us(us: u32) -> Self::Ticks {
        let val: u64 = Self::Frequency::frequency() as u64 * us as u64;
        ticks_from_val(val / 1_000_000)
    }
}

fn ticks_from_val<T: Ticks>(val: u64) -> T {
    if val <= T::max_value().into_u32() as u64 {
        T::from(val as u32)
    } else {
        T::max_value()
    }
}

/// Represents a static moment in time, that does not change over
/// repeated calls to `Time::now`.
pub trait Timestamp: Time {}

/// Callback handler for when a counter has overflowed past its maximum
/// value and returned to 0.
pub trait OverflowClient {
    fn overflow(&self);
}

/// Represents a free-running hardware counter that can be started and stopped.
pub trait Counter<'a>: Time {
    /// Specify the callback for when the counter overflows its maximum
    /// value (defined by `Ticks`). If there was a previously registered
    /// callback this call replaces it.
    fn set_overflow_client(&'a self, client: &'a dyn OverflowClient);

    /// Starts the free-running hardware counter. Valid `Result<(), ErrorCode>` values are:
    ///   - `Ok(())`: the counter is now running
    ///   - `Err(ErrorCode::OFF)`: underlying clocks or other hardware resources
    ///   are not on, such that the counter cannot start.
    ///   - `Err(ErrorCode::FAIL)`: unidentified failure, counter is not running.
    /// After a successful call to `start`, `is_running` MUST return true.    
    fn start(&self) -> Result<(), ErrorCode>;

    /// Stops the free-running hardware counter. Valid `Result<(), ErrorCode>` values are:
    ///   - `Ok(())`: the counter is now stopped. No further
    ///   overflow callbacks will be invoked.
    ///   - `Err(ErrorCode::BUSY)`: the counter is in use in a way that means it
    ///   cannot be stopped and is busy.
    ///   - `Err(ErrorCode::FAIL)`: unidentified failure, counter is running.
    /// After a successful call to `stop`, `is_running` MUST return false.        
    fn stop(&self) -> Result<(), ErrorCode>;

    /// Resets the counter to 0. This may introduce jitter on the counter.
    /// Resetting the counter has no effect on any pending overflow callbacks.
    /// If a client needs to reset and clear pending callbacks it should
    /// call `stop` before `reset`.
    /// Valid `Result<(), ErrorCode>` values are:
    ///    - `Ok(())`: the counter was reset to 0.
    ///    - `Err(ErrorCode::FAIL)`: the counter was not reset to 0.    
    fn reset(&self) -> Result<(), ErrorCode>;

    /// Returns whether the counter is currently running.
    fn is_running(&self) -> bool;
}

/// Callback handler for when an Alarm fires (a `Counter` reaches a specific
/// value).
pub trait AlarmClient {
    /// Callback indicating the alarm time has been reached. The alarm
    /// MUST be disabled when this is called. If a new alarm is needed,
    /// the client can call `Alarm::set_alarm`.
    fn alarm(&self);
}

/// Interface for receiving notification when a particular time
/// (`Counter` value) is reached. Clients use the
/// [`AlarmClient`](trait.AlarmClient.html) trait to signal when the
/// counter has reached a pre-specified value set in
/// [`set_alarm`](#tymethod.set_alarm). Alarms are intended for
/// low-level time needs that require precision (i.e., firing on a
/// precise clock tick). Software that needs more functionality
/// but can tolerate some jitter should use the `Timer` trait
/// instead.
pub trait Alarm<'a>: Time {
    /// Specify the callback for when the counter reaches the alarm
    /// value. If there was a previously installed callback this call
    /// replaces it.
    fn set_alarm_client(&'a self, client: &'a dyn AlarmClient);

    /// Specify when the callback should be called and enable it. The
    /// callback will be enqueued when `Time::now() == reference + dt`. The
    /// callback itself may not run exactly at this time, due to delays.
    /// However, it it assured to execute *after* `reference + dt`: it can
    /// be delayed but will never fire early. The method takes `reference`
    /// and `dt` rather than a single value denoting the counter value so it
    /// can distinguish between alarms which have very recently already
    /// passed and those in the far far future (see #1651).
    fn set_alarm(&self, reference: Self::Ticks, dt: Self::Ticks);

    /// Return the current alarm value. This is undefined at boot and
    /// otherwise returns `now + dt` from the last call to `set_alarm`.
    fn get_alarm(&self) -> Self::Ticks;

    /// Disable the alarm and stop it from firing in the future.
    /// Valid `Result<(), ErrorCode>` codes are:
    ///   - `Ok(())` the alarm has been disarmed and will not invoke
    ///   the callback in the future    
    ///   - `Err(ErrorCode::FAIL)` the alarm could not be disarmed and will invoke
    ///   the callback in the future    
    fn disarm(&self) -> Result<(), ErrorCode>;

    /// Returns whether the alarm is currently armed. Note that this
    /// does not reliably indicate whether there will be a future
    /// callback: it is possible that the alarm has triggered (and
    /// disarmed) and a callback is pending and has not been called yet.
    /// In this case it possible for `is_armed` to return false yet to
    /// receive a callback.
    fn is_armed(&self) -> bool;

    /// Return the minimum dt value that is supported. Any dt smaller than
    /// this will automatically be increased to this minimum value.
    fn minimum_dt(&self) -> Self::Ticks;
}

/// Callback handler for when a timer fires.
pub trait TimerClient {
    fn timer(&self);
}

/// Interface for controlling callbacks when an interval has passed.
/// This interface is intended for software that requires repeated
/// and/or one-shot timers and is willing to experience some jitter or
/// imprecision in return for a simpler API that doesn't require
/// actual calculation of counter values. Software that requires more
/// precisely timed callbacks should use the `Alarm` trait instead.
pub trait Timer<'a>: Time {
    /// Specify the callback to invoke when the timer interval expires.
    /// If there was a previously installed callback this call replaces it.    
    fn set_timer_client(&'a self, client: &'a dyn TimerClient);

    /// Start a one-shot timer that will invoke the callback at least
    /// `interval` ticks in the future. If there is a timer currently pending,
    /// calling this cancels that previous timer. After a callback is invoked
    /// for a one shot timer, the timer MUST NOT invoke the callback again
    /// unless a new timer is started (either with repeating or one shot).
    /// Returns the actual interval for the timer that was registered.
    /// This MUST NOT be smaller than `interval` but MAY be larger.
    fn oneshot(&'a self, interval: Self::Ticks) -> Self::Ticks;

    /// Start a repeating timer that will invoke the callback every
    /// `interval` ticks in the future. If there is a timer currently
    /// pending, calling this cancels that previous timer.
    /// Returns the actual interval for the timer that was registered.
    /// This MUST NOT be smaller than `interval` but MAY be larger.
    fn repeating(&'a self, interval: Self::Ticks) -> Self::Ticks;

    /// Return the interval of the last requested timer.
    fn interval(&self) -> Option<Self::Ticks>;

    /// Return if the last requested timer is a one-shot timer.
    fn is_oneshot(&self) -> bool;

    /// Return if the last requested timer is a repeating timer.
    fn is_repeating(&self) -> bool;

    /// Return how many ticks are remaining until the next callback,
    /// or None if the timer is disabled.  This call is useful because
    /// there may be non-neglible delays between when a timer was
    /// requested and it was actually scheduled. Therefore, since a
    /// timer's start might be delayed slightly, the time remaining
    /// might be slightly higher than one would expect if one
    /// calculated it right before the call to start the timer.
    fn time_remaining(&self) -> Option<Self::Ticks>;

    /// Returns whether there is currently a timer enabled and so a callback
    /// will be expected in the future. If `is_enabled` returns false then
    /// the implementation MUST NOT invoke a callback until a call to `oneshot`
    /// or `repeating` restarts the timer.
    fn is_enabled(&self) -> bool;

    /// Cancel the current timer, if any. Value `Result<(), ErrorCode>` values are:
    ///  - `Ok(())`: no callback will be invoked in the future.
    ///  - `Err(ErrorCode::FAIL)`: the timer could not be cancelled and a callback
    ///  will be invoked in the future.
    fn cancel(&self) -> Result<(), ErrorCode>;
}

/// 100MHz `Frequency`
#[derive(Debug)]
pub struct Freq100MHz;
impl Frequency for Freq100MHz {
    fn frequency() -> u32 {
        100000000
    }
}

/// 16MHz `Frequency`
#[derive(Debug)]
pub struct Freq16MHz;
impl Frequency for Freq16MHz {
    fn frequency() -> u32 {
        16000000
    }
}

/// 1MHz `Frequency`
#[derive(Debug)]
pub struct Freq1MHz;
impl Frequency for Freq1MHz {
    fn frequency() -> u32 {
        1000000
    }
}

/// 32KHz `Frequency`
#[derive(Debug)]
pub struct Freq32KHz;
impl Frequency for Freq32KHz {
    fn frequency() -> u32 {
        32768
    }
}

/// 16KHz `Frequency`
#[derive(Debug)]
pub struct Freq16KHz;
impl Frequency for Freq16KHz {
    fn frequency() -> u32 {
        16000
    }
}

/// 1KHz `Frequency`
#[derive(Debug)]
pub struct Freq1KHz;
impl Frequency for Freq1KHz {
    fn frequency() -> u32 {
        1000
    }
}

/// u32 `Ticks`
#[derive(Clone, Copy, Debug)]
pub struct Ticks32(u32);

impl From<u32> for Ticks32 {
    fn from(val: u32) -> Self {
        Ticks32(val)
    }
}

impl Ticks for Ticks32 {
    fn into_usize(self) -> usize {
        self.0 as usize
    }

    fn into_u32(self) -> u32 {
        self.0
    }

    fn wrapping_add(self, other: Self) -> Self {
        Ticks32(self.0.wrapping_add(other.0))
    }

    fn wrapping_sub(self, other: Self) -> Self {
        Ticks32(self.0.wrapping_sub(other.0))
    }

    fn within_range(self, start: Self, end: Self) -> bool {
        self.wrapping_sub(start).0 < end.wrapping_sub(start).0
    }

    /// Returns the maximum value of this type, which should be (2^width)-1.
    fn max_value() -> Self {
        Ticks32(0xFFFFFFFF)
    }
}

impl PartialOrd for Ticks32 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ticks32 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialEq for Ticks32 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Ticks32 {}

/// 24-bit `Ticks`
#[derive(Clone, Copy, Debug)]
pub struct Ticks24(u32);

impl From<u32> for Ticks24 {
    fn from(val: u32) -> Self {
        Ticks24(val)
    }
}

impl Ticks for Ticks24 {
    fn into_usize(self) -> usize {
        self.0 as usize
    }

    fn into_u32(self) -> u32 {
        self.0
    }

    fn wrapping_add(self, other: Self) -> Self {
        Ticks24(self.0.wrapping_add(other.0) & 0x00FFFFFF)
    }

    fn wrapping_sub(self, other: Self) -> Self {
        Ticks24(self.0.wrapping_sub(other.0) & 0x00FFFFFF)
    }

    fn within_range(self, start: Self, end: Self) -> bool {
        self.wrapping_sub(start).0 < end.wrapping_sub(start).0
    }

    /// Returns the maximum value of this type, which should be (2^width)-1.
    fn max_value() -> Self {
        Ticks24(0x00FFFFFF)
    }
}

impl PartialOrd for Ticks24 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ticks24 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialEq for Ticks24 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Ticks24 {}

/// 16-bit `Ticks`
#[derive(Clone, Copy, Debug)]
pub struct Ticks16(u16);

impl From<u16> for Ticks16 {
    fn from(val: u16) -> Self {
        Ticks16(val)
    }
}

impl From<u32> for Ticks16 {
    fn from(val: u32) -> Self {
        Ticks16((val & 0xffff) as u16)
    }
}

impl Ticks16 {
    pub fn into_u16(self) -> u16 {
        self.0
    }
}

impl Ticks for Ticks16 {
    fn into_usize(self) -> usize {
        self.0 as usize
    }

    fn into_u32(self) -> u32 {
        self.0 as u32
    }

    fn wrapping_add(self, other: Self) -> Self {
        Ticks16(self.0.wrapping_add(other.0))
    }

    fn wrapping_sub(self, other: Self) -> Self {
        Ticks16(self.0.wrapping_sub(other.0))
    }

    fn within_range(self, start: Self, end: Self) -> bool {
        self.wrapping_sub(start).0 < end.wrapping_sub(start).0
    }

    /// Returns the maximum value of this type, which should be (2^width)-1.
    fn max_value() -> Self {
        Ticks16(0xFFFF)
    }
}

impl PartialOrd for Ticks16 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ticks16 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialEq for Ticks16 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Ticks16 {}

/// 64-bit `Ticks`
#[derive(Clone, Copy, Debug)]
pub struct Ticks64(u64);

impl Ticks64 {
    pub fn into_u64(self) -> u64 {
        self.0
    }
}

impl From<u32> for Ticks64 {
    fn from(val: u32) -> Self {
        Ticks64(val as u64)
    }
}

impl From<u64> for Ticks64 {
    fn from(val: u64) -> Self {
        Ticks64(val as u64)
    }
}

impl Ticks for Ticks64 {
    fn into_usize(self) -> usize {
        self.0 as usize
    }

    fn into_u32(self) -> u32 {
        self.0 as u32
    }

    fn wrapping_add(self, other: Self) -> Self {
        Ticks64(self.0.wrapping_add(other.0))
    }

    fn wrapping_sub(self, other: Self) -> Self {
        Ticks64(self.0.wrapping_sub(other.0))
    }

    fn within_range(self, start: Self, end: Self) -> bool {
        self.wrapping_sub(start).0 < end.wrapping_sub(start).0
    }

    /// Returns the maximum value of this type, which should be (2^width)-1.
    fn max_value() -> Self {
        Ticks64(!0u64)
    }
}

impl PartialOrd for Ticks64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ticks64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialEq for Ticks64 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Ticks64 {}
