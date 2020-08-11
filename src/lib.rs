//! Debounce a noisy digital input signal.
//!
//! Even digital input signals can be noisy.  The example usually cited
//! is the flapping of physical contacts in a button or switch, but RF
//! line noise can also cause digital input signals to bounce.  Robust
//! devices and embedded systems must debounce inputs.
//!
//! This crate is a batteries-included [`embedded-hal`][0] `InputPin`
//! debouncer, using the integration-based algorithm described by
//! Kenneth A. Kuhn in [a code sample on his website][1].  You are
//! highly recommended to read the code comments there.
//!
//! - [Documentation](https://docs.rs/debounced)
//! - [Repository](https://git.sr.ht/~couchand/debounced)
//!
//! # Minimum supported Rust version
//!
//! This crate makes use of trait bounds on a `const fn`, which is
//! currently unstable.  Therefore, we require use of the nightly
//! compiler.  When [rust-lang/rust#67792][2] stabilizes, we will
//! establish a MSRV policy.
//!
//! # Usage
//!
//! You need to bring just a few things:
//!
//! - An [`InputPin`][3], perhaps provided by a peripheral access crate
//!   (PAC) or hardware abstraction layer (HAL) for your chip.
//! - An implementation of the [`Debounce`](Debounce) trait, maybe just
//!   one from the [`default`](default) module.
//! - Some way to regularly call the [`poll()`](Debouncer#method.poll)
//!   method at about the right frequency (where "right" means "roughly
//!   consistent with the assumptions made in the `Debounce` trait
//!   implementation").  This may be an interrupt service routine (ISR),
//!   or it could just be a spin-delayed call from your main loop.
//! - Storage for the debounce state.  If you're using an ISR for
//!   polling, you'll want this to be a `static`.
//!
//! Once you've worked out these details,  the `debounced` crate will
//! take care of the rest.
//!
//! ```toml
//! [dependencies]
//! debounced = "0.1"
//! ```
//!
//! Your implementation will consist of three major steps:
//!
//! ## Create the debouncer.
//!
//! If you're storing state in a `static`, that might be:
//!
//! ```
//! # struct PinType;
//! # impl embedded_hal::digital::v2::InputPin for PinType {
//! #     type Error = core::convert::Infallible;
//! #     fn is_high(&self) -> Result<bool, Self::Error> {
//! #         Ok(true)
//! #     }
//! #     fn is_low(&self) -> Result<bool, Self::Error> {
//! #         Ok(false)
//! #     }
//! # }
//! use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
//! static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
//! ```
//!
//! ## Initialize the debouncer.
//!
//! Next, initialize the [`Debouncer`](Debouncer).  You pass in the
//! input pin and get back the debounced pin.  If you're storing state
//! in a `static`, that might look like this:
//!
//! ```
//! # struct PinType;
//! # impl embedded_hal::digital::v2::InputPin for PinType {
//! #     type Error = core::convert::Infallible;
//! #     fn is_high(&self) -> Result<bool, Self::Error> {
//! #         Ok(true)
//! #     }
//! #     fn is_low(&self) -> Result<bool, Self::Error> {
//! #         Ok(false)
//! #     }
//! # }
//! # use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
//! # static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
//! # let input_pin = PinType;
//! let debounced_pin = unsafe { DEBOUNCER.init(input_pin) }.unwrap();
//! ```
//!
//! See the docs on the [`init()`](Debounce#method.init) method for
//! safety details.  Generally, if you haven't yet enabled interrupts
//! you'll be fine.
//!
//! ## Poll the debouncer.
//!
//! On a regular basis, make a call to the [`poll()`](Debouncer#method.poll)
//! method of `Debouncer`, which might look like this:
//!
//! ```
//! # struct PinType;
//! # impl embedded_hal::digital::v2::InputPin for PinType {
//! #     type Error = core::convert::Infallible;
//! #     fn is_high(&self) -> Result<bool, Self::Error> {
//! #         Ok(true)
//! #     }
//! #     fn is_low(&self) -> Result<bool, Self::Error> {
//! #         Ok(false)
//! #     }
//! # }
//! # use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
//! # static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
//! # let input_pin = PinType;
//! # let _ = unsafe { DEBOUNCER.init(input_pin) }.unwrap();
//! unsafe {
//!     DEBOUNCER.poll().unwrap();
//! }
//! ```
//!
//! Again, see the docs on the relevant method for safety information.
//! The main idea here is that you should only ever `poll()` from one
//! place in your code.  We'd use a `&mut` reference, but, well, it's
//! in static storage.
//!
//! [0]: https://github.com/rust-embedded/embedded-hal
//! [1]: http://www.kennethkuhn.com/electronics/debounce.c
//! [2]: https://github.com/rust-lang/rust/issues/67792
//! [3]: https://docs.rs/embedded-hal/0.2.4/embedded_hal/digital/v2/trait.InputPin.html

#![no_std]
#![deny(missing_docs)]
#![feature(const_fn)]
#![doc(html_root_url = "https://docs.rs/debounced/0.1.0")]

use core::cell::UnsafeCell;
use core::convert::Infallible;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, Not, Shl, Shr, SubAssign};

use embedded_hal::digital::v2::InputPin;

/// Static configuration of the debouncing algorithm.
pub trait Debounce {
    /// The storage type of the state.  For most usages, `u8` is plenty
    /// big enough.  You almost certainly don't need more than a `u8`.
    type Storage: From<u8>
        + BitAnd<Output = Self::Storage>
        + BitAndAssign
        + BitOr<Output = Self::Storage>
        + BitOrAssign
        + Not<Output = Self::Storage>
        + Shl<u8, Output = Self::Storage>
        + Shr<u8, Output = Self::Storage>
        + AddAssign
        + SubAssign
        + Eq
        + Copy;

    /// The number of samples required to mark a state change.
    ///
    /// Unlike many debouncing algorithms, the integration approach
    /// doesn't require a fixed number of consistent samples in a row.
    /// Rather, if in `n + m` samples we see `n` of the new state and
    /// `m` of the old state, a transition will be marked if the
    /// difference `n - m` reaches `MAX_COUNT`.
    ///
    /// This should be configured based on the following formula: if
    /// `d` is the minimum debounce delay (in seconds), and `f` is the
    /// number of times the debouncer is polled per second, the
    /// `MAX_COUNT` should be set to the product `d * f`. For instance,
    /// if polling 100 times a second (100 Hz), with a minimum delay of
    /// 50 milliseconds, set this to 5.
    ///
    /// *Note:* this must be non zero, and must be represented in two
    /// bits fewer than the storage you provide (e.g. if using `u8`,
    /// `MAX_COUNT` cannot exceed `0x3f`.  For the algorithm to perform
    /// any meaningful debouncing, it must be greater than 1.
    const MAX_COUNT: Self::Storage;

    /// The initial state of the pin.
    ///
    /// If `INIT_HIGH` is true, the debounced pin will start high and
    /// wait for the first falling edge.  If this is false, the pin
    /// will start low and wait for the first debounced rising edge.
    const INIT_HIGH: bool;
}

trait DebounceExt: Debounce {
    fn zero() -> Self::Storage;
    fn state_mask() -> Self::Storage;
    fn init_mask() -> Self::Storage;
    fn integrator_mask() -> Self::Storage;
    fn integrator_one() -> Self::Storage;
    fn integrator_max() -> Self::Storage;
}

impl<D: Debounce> DebounceExt for D {
    #[inline(always)]
    fn zero() -> Self::Storage {
        Self::Storage::from(0)
    }

    #[inline(always)]
    fn state_mask() -> Self::Storage {
        Self::Storage::from(1)
    }

    #[inline(always)]
    fn init_mask() -> Self::Storage {
        Self::Storage::from(1 << 1)
    }

    #[inline(always)]
    fn integrator_mask() -> Self::Storage {
        let mut mask = Self::integrator_one();
        mask -= Self::Storage::from(1);
        !mask
    }

    #[inline(always)]
    fn integrator_one() -> Self::Storage {
        Self::Storage::from(1 << 2)
    }

    #[inline(always)]
    fn integrator_max() -> Self::Storage {
        Self::MAX_COUNT << 2
    }
}

/// Some default configurations.
///
/// These provide reasonable defaults for the common case of debouncing
/// the contact flapping on a physical button or switch.
pub mod default {
    /// A reasonable default active-high configuration.
    ///
    /// If the debounced pin is polled every 10ms (100Hz), the minimum
    /// debounce delay is 40ms.
    pub struct ActiveHigh;

    impl super::Debounce for ActiveHigh {
        /// For most usages, `u8` is plenty.
        type Storage = u8;

        /// With a `MAX_COUNT` of 4, the minimum delay is 40ms at 100Hz.
        const MAX_COUNT: Self::Storage = 4;

        /// Since the switch is active high, `INIT_HIGH` is false.
        const INIT_HIGH: bool = false;
    }

    /// A reasonable default active-low configuration.
    ///
    /// If the debounced pin is polled every 10ms (100Hz), the minimum
    /// debounce delay is 40ms.
    pub struct ActiveLow;

    impl super::Debounce for ActiveLow {
        /// For most usages, `u8` is plenty.
        type Storage = u8;

        /// With a `MAX_COUNT` of 4, the minimum delay is 40ms at 100Hz.
        const MAX_COUNT: Self::Storage = 4;

        /// Since the switch is active low, `INIT_HIGH` is true.
        const INIT_HIGH: bool = true;
    }

    /// The settings in Kenneth A. Kuhn's [code fragment][0].
    ///
    /// If the debounced pin is polled every 100ms (10Hz), the minimum
    /// delay is 300ms.
    ///
    /// [0]: http://www.kennethkuhn.com/electronics/debounce.c
    pub struct OriginalKuhn;

    impl super::Debounce for OriginalKuhn {
        /// For most usages, `u8` is plenty.
        type Storage = u8;

        /// With a `MAX_COUNT` of 3, the minimum delay is 300ms at 10Hz.
        const MAX_COUNT: Self::Storage = 3;

        /// Kuhn's code fragment doesn't included initialization code,
        /// so we've just used a default of false consistent with the
        /// comments.
        const INIT_HIGH: bool = false;
    }
}

/// An error indicating that once-only initialization has been violated.
#[derive(Debug)]
pub struct InitError;

/// An error that arose during polling.
#[derive(Debug)]
pub enum PollError<PinError> {
    /// The `Debouncer` was polled before the call to
    /// [`init()`](Debouncer#method.init) completed.
    Init,

    /// An error polling the underlying pin.
    Pin(PinError),
}

/// An error that arose during deinit.
pub enum DeinitError<'a, Cfg: Debounce> {
    /// The `Debouncer` was not initialized.
    Init,

    /// The provided pin does not match this `Debouncer`.
    Pin(Debounced<'a, Cfg>),
}

impl<'a, Cfg: Debounce> core::fmt::Debug for DeinitError<'a, Cfg> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DeinitError::Init => f.write_str("Init"),
            DeinitError::Pin(_) => f.write_str("Pin(_)"),
        }
    }
}

/// A pin debouncer.
///
/// Since this needs to be shared between the main application code and
/// the interupt service routine, it is generally put into a static.
///
/// For technical reasons the zero value of the storage type
/// [`Debounce::Storage`](Debounce#associatedtype.Storage) must be
/// provided this type's constructor.  For this reason, the preferred
/// way to create a `Debouncer` is with the macro
/// [`debouncer_uninit!`](debouncer_uninit).
///
/// ```
/// # struct PinType;
/// # impl embedded_hal::digital::v2::InputPin for PinType {
/// #     type Error = core::convert::Infallible;
/// #     fn is_high(&self) -> Result<bool, Self::Error> {
/// #         Ok(true)
/// #     }
/// #     fn is_low(&self) -> Result<bool, Self::Error> {
/// #         Ok(false)
/// #     }
/// # }
/// use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
/// static PIN_DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
/// ```
///
/// Later, in your main application code, you can initialize it with
/// the relevant input pin.  This returns the [`Debounced`](Debounced)
/// pin for your use.
///
/// ```
/// # struct PinType;
/// # impl embedded_hal::digital::v2::InputPin for PinType {
/// #     type Error = core::convert::Infallible;
/// #     fn is_high(&self) -> Result<bool, Self::Error> {
/// #         Ok(true)
/// #     }
/// #     fn is_low(&self) -> Result<bool, Self::Error> {
/// #         Ok(false)
/// #     }
/// # }
/// # use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
/// # static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
/// # let input_pin = PinType;
/// let debounced_pin = unsafe { DEBOUNCER.init(input_pin) }.unwrap();
/// ```
///
/// Finally, make sure to arrange for regular polling of the `Debouncer`.
///
/// ```
/// # struct PinType;
/// # impl embedded_hal::digital::v2::InputPin for PinType {
/// #     type Error = core::convert::Infallible;
/// #     fn is_high(&self) -> Result<bool, Self::Error> {
/// #         Ok(true)
/// #     }
/// #     fn is_low(&self) -> Result<bool, Self::Error> {
/// #         Ok(false)
/// #     }
/// # }
/// # use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
/// # static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
/// # let input_pin = PinType;
/// # let _ = unsafe { DEBOUNCER.init(input_pin) }.unwrap();
/// unsafe {
///     DEBOUNCER.poll().unwrap();
/// }
/// ```
pub struct Debouncer<Pin, Cfg: Debounce> {
    cfg: PhantomData<Cfg>,
    pin: UnsafeCell<MaybeUninit<Pin>>,
    storage: UnsafeCell<Cfg::Storage>,
}

// We demand particular mutex requirements as documented on the methods
// marked as unsafe.  They are expected to be enforced statically by
// the user, outside of the type system.
unsafe impl<Pin, Cfg: Debounce> Sync for Debouncer<Pin, Cfg> {}

impl<Pin: InputPin, Cfg: Debounce> Debouncer<Pin, Cfg> {
    /// Initialize the pin debouncer for a given input pin.
    ///
    /// Returns an error if the `Debouncer` has already be initialized.
    ///
    /// # Safety
    ///
    /// For this call to be safe, you must ensure that it is not run
    /// concurrently with a call to any unsafe method of this type,
    /// including `init()` itself. The usual way to do this is by
    /// calling `init()` once before enabling interrupts.
    ///
    /// # Examples
    ///
    /// ```
    /// # struct PinType;
    /// # impl embedded_hal::digital::v2::InputPin for PinType {
    /// #     type Error = core::convert::Infallible;
    /// #     fn is_high(&self) -> Result<bool, Self::Error> {
    /// #         Ok(true)
    /// #     }
    /// #     fn is_low(&self) -> Result<bool, Self::Error> {
    /// #         Ok(false)
    /// #     }
    /// # }
    /// # use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
    /// # static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
    /// # let input_pin = PinType;
    /// let debounced_pin = unsafe { DEBOUNCER.init(input_pin) }.unwrap();
    /// ```
    #[inline]
    pub unsafe fn init(&self, pin: Pin) -> Result<Debounced<Cfg>, InitError> {
        // TODO: this would be great as a static assert if we could.
        assert!(
            (Cfg::MAX_COUNT << 2) >> 2 == Cfg::MAX_COUNT,
            "Debounce::MAX_COUNT must be represented in two bits fewer than Debounce::Storage"
        );

        self.init_linted(pin)
    }

    // n.b. defined seperately to ensure that we think about unsafety.
    #[inline(always)]
    fn init_linted(&self, pin: Pin) -> Result<Debounced<Cfg>, InitError> {
        if self.init_flag() {
            return Err(InitError);
        }

        let pin_cell_ptr = self.pin.get();
        // This is safe because we demand from the caller that this
        // method completes before any call to `poll()`.
        let pin_cell = unsafe { &mut *pin_cell_ptr };

        let pin_ptr = pin_cell.as_mut_ptr();
        // It is always safe to write to a MaybeUninit pointer.
        unsafe {
            pin_ptr.write(pin);
        }

        // TODO: should this be moved to intepretation side?
        let mut new_state = if Cfg::INIT_HIGH {
            Cfg::state_mask() | Cfg::integrator_max()
        } else {
            Cfg::zero()
        };
        new_state |= Cfg::init_mask();

        let state_ptr = self.storage.get();
        // This is safe because we demand from the caller that this
        // method completes before any call to `poll()`.
        unsafe {
            *state_ptr = new_state;
        }

        Ok(Debounced {
            cfg: PhantomData,
            storage: &self.storage,
        })
    }

    /// Poll the pin debouncer.
    ///
    /// This should be done on a regular basis at roughly the frequency
    /// used in the calculation of [`MAX_COUNT`](Debounce#associatedconstant.MAX_COUNT).
    ///
    /// # Safety
    ///
    /// For this method to be safe, you must ensure that it is not run
    /// concurrently with a call to any unsafe method of this type,
    /// including `poll()` itself.  The usual way to do this is to call
    /// `poll()` from a single interrupt service routine, and not
    /// enable interrupts until after the call to `init()` returns.
    ///
    /// # Examples
    ///
    /// ```
    /// # struct PinType;
    /// # impl embedded_hal::digital::v2::InputPin for PinType {
    /// #     type Error = core::convert::Infallible;
    /// #     fn is_high(&self) -> Result<bool, Self::Error> {
    /// #         Ok(true)
    /// #     }
    /// #     fn is_low(&self) -> Result<bool, Self::Error> {
    /// #         Ok(false)
    /// #     }
    /// # }
    /// # use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
    /// # static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
    /// # let input_pin = PinType;
    /// # let _ = unsafe { DEBOUNCER.init(input_pin) }.unwrap();
    /// unsafe {
    ///     DEBOUNCER.poll().unwrap();
    /// }
    /// ```
    #[inline]
    pub unsafe fn poll(&self) -> Result<(), PollError<Pin::Error>> {
        // TODO: can we make this safe with a mutex bit?
        // is that hair-brained? hare-brained? whatever

        self.poll_linted()
    }

    // n.b. defined seperately to ensure that we think about unsafety.
    #[inline(always)]
    fn poll_linted(&self) -> Result<(), PollError<Pin::Error>> {
        if !self.init_flag() {
            return Err(PollError::Init);
        }

        let pin_cell_ptr = self.pin.get();
        // This is safe because we only ever mutate in `init()`.
        let pin_cell = unsafe { &*pin_cell_ptr };

        let pin_ptr = pin_cell.as_ptr();
        // This is safe because we've checked that init has completed.
        let pin = unsafe { &*pin_ptr };

        if pin.is_low().map_err(PollError::Pin)? {
            self.decrement_integrator();

            if self.integrator_is_zero() {
                self.clear_state_flag();
            }
        } else {
            // TODO: should this check if pin is high?
            self.increment_integrator();

            if self.integrator_is_max() {
                self.set_state_flag();
            }
        }

        Ok(())
    }

    /// Create a new, uninitialized pin debouncer.
    ///
    /// For technical reasons, you must pass in the zero value of the
    /// storage type [`Debounce::Storage`](Debounce#associatedtype.Storage),
    /// so prefer the macro [`debouncer_uninit!`](debouncer_uninit).
    pub const fn uninit(zero: Cfg::Storage) -> Self {
        Debouncer {
            cfg: PhantomData,
            pin: UnsafeCell::new(MaybeUninit::uninit()),
            storage: UnsafeCell::new(zero),
        }
    }

    /// Destroy the debounced pin, returning the original input pin.
    ///
    /// You must pass in the debounced pin produced from the call to
    /// [`init()`](#method.init).  Returns an error if called with a
    /// `Debounced` pin not associated with this `Debouncer`.
    ///
    /// Restores this `Debouncer` to the uninitialized state.
    ///
    /// # Safety
    ///
    /// For this method to be safe, you must ensure that it is not run
    /// concurrently with a call to any unsafe method of this type,
    /// including `deinit()` itself.
    ///
    /// If you only ever `poll()` in an interrupt service routine, you
    /// call this method in main application code, your architecture
    /// guarantees that main application code never preempts an
    /// interrupt service routine, and you disable interrupts before
    /// calling it, this will be safe.
    #[inline]
    pub unsafe fn deinit<'a>(&self, pin: Debounced<'a, Cfg>) -> Result<Pin, DeinitError<'a, Cfg>> {
        self.deinit_linted(pin)
    }

    // n.b. defined seperately to ensure that we think about unsafety.
    #[inline(always)]
    fn deinit_linted<'a>(&self, pin: Debounced<'a, Cfg>) -> Result<Pin, DeinitError<'a, Cfg>> {
        if !self.init_flag() {
            return Err(DeinitError::Init);
        }

        if self.storage.get() != pin.storage.get() {
            return Err(DeinitError::Pin(pin));
        }

        let state_ptr = self.storage.get();
        // This is safe because we demand from the caller that it not
        // interrupt or be interrupted by a call to `poll()`.
        unsafe {
            *state_ptr = Cfg::zero();
        }

        // Ensure no aliasing.
        let pin = {
            let pin_cell_ptr = self.pin.get();
            // This is safe because we demand from the caller that this is
            // an exclusive call.
            let pin_cell = unsafe { &*pin_cell_ptr };

            let pin_ptr = pin_cell.as_ptr();
            // This is safe because we've checked the init flag above.
            unsafe { pin_ptr.read() }
        };

        let pin_cell_ptr = self.pin.get();
        // This is safe because we've demanded no aliasing.
        unsafe {
            *pin_cell_ptr = MaybeUninit::uninit();
        }

        Ok(pin)
    }

    #[inline]
    fn init_flag(&self) -> bool {
        let state_ptr = self.storage.get();
        // This is safe because the read is atomic.
        let state = unsafe { *state_ptr };

        state & Cfg::init_mask() != Cfg::zero()
    }

    #[inline(always)]
    fn set_state_flag(&self) {
        let state_ptr = self.storage.get();

        // This is safe since we're the only ones allowed to mutate.
        unsafe {
            *state_ptr |= Cfg::state_mask();
        }
    }

    #[inline(always)]
    fn clear_state_flag(&self) {
        let state_ptr = self.storage.get();

        // This is safe since we're the only ones allowed to mutate.
        unsafe {
            *state_ptr &= !Cfg::state_mask();
        }
    }

    #[inline(always)]
    fn integrator_is_zero(&self) -> bool {
        let state_ptr = self.storage.get();

        // This is safe since the read is atomic.
        let state = unsafe { *state_ptr };
        let integrator = state & Cfg::integrator_mask();
        integrator == Cfg::zero()
    }

    #[inline(always)]
    fn integrator_is_max(&self) -> bool {
        let state_ptr = self.storage.get();

        // This is safe since the read is atomic.
        let state = unsafe { *state_ptr };
        let integrator = state & Cfg::integrator_mask();
        integrator == Cfg::integrator_max()
    }

    #[inline(always)]
    fn decrement_integrator(&self) {
        let state_ptr = self.storage.get();

        // This is safe since we're the only ones allowed to mutate.
        if !self.integrator_is_zero() {
            unsafe {
                *state_ptr -= Cfg::integrator_one();
            }
        }
    }

    #[inline(always)]
    fn increment_integrator(&self) {
        let state_ptr = self.storage.get();

        // This is safe since we're the only ones allowed to mutate.
        if !self.integrator_is_max() {
            unsafe {
                *state_ptr += Cfg::integrator_one();
            }
        }
    }
}

/// Create a new uninitialized [`Debouncer`](Debouncer).
///
/// This is the preferred way to initialize a static `Debouncer`.  Be
/// sure to initialize it before doing anything else with it, or you'll
/// get an error `Result`.
///
/// # Examples
///
/// ```
/// # struct PinType;
/// # impl embedded_hal::digital::v2::InputPin for PinType {
/// #     type Error = core::convert::Infallible;
/// #     fn is_high(&self) -> Result<bool, Self::Error> {
/// #         Ok(true)
/// #     }
/// #     fn is_low(&self) -> Result<bool, Self::Error> {
/// #         Ok(false)
/// #     }
/// # }
/// use debounced::{debouncer_uninit, Debouncer, default::ActiveLow};
/// static PIN_DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
/// ```
#[macro_export]
macro_rules! debouncer_uninit {
    () => {
        $crate::Debouncer::uninit(0)
    };
}

/// A debounced pin.
///
/// This is what you'll use for downstream input processing, leveraging
/// the methods provided by the trait [`InputPin`](#impl-InputPin).
pub struct Debounced<'state, Cfg: Debounce> {
    cfg: PhantomData<Cfg>,
    storage: &'state UnsafeCell<Cfg::Storage>,
}

impl<'state, Cfg: Debounce> InputPin for Debounced<'state, Cfg> {
    type Error = Infallible;

    #[inline(always)]
    fn is_high(&self) -> Result<bool, Self::Error> {
        let state_ptr = self.storage.get();
        // This is safe since the read is atomic.
        let state = unsafe { *state_ptr };
        let flag = state & Cfg::state_mask();
        Ok(flag != Cfg::zero())
    }

    #[inline(always)]
    fn is_low(&self) -> Result<bool, Self::Error> {
        let state_ptr = self.storage.get();
        // This is safe since the read is atomic.
        let state = unsafe { *state_ptr };
        let flag = state & Cfg::state_mask();
        Ok(flag == Cfg::zero())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use embedded_hal_mock::pin;

    #[test]
    fn simple() {
        struct Cfg;
        impl Debounce for Cfg {
            type Storage = u8;
            const MAX_COUNT: u8 = 3;
            const INIT_HIGH: bool = false;
        }

        let expectations = [
            pin::Transaction::get(pin::State::High),
            pin::Transaction::get(pin::State::High),
            pin::Transaction::get(pin::State::High),
            pin::Transaction::get(pin::State::Low),
            pin::Transaction::get(pin::State::Low),
            pin::Transaction::get(pin::State::Low),
        ];

        let pin = pin::Mock::new(&expectations);

        let debouncer: Debouncer<_, Cfg> = debouncer_uninit!();
        // It is always safe to init a stack-scoped Debouncer.
        let debounced = unsafe { debouncer.init(pin) }.expect("debounced pin");

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        // It is always safe to poll a stack-scoped Debouncer.
        unsafe { debouncer.poll() }.unwrap();

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        // It is always safe to poll a stack-scoped Debouncer.
        unsafe { debouncer.poll() }.unwrap();

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        // It is always safe to poll a stack-scoped Debouncer.
        unsafe { debouncer.poll() }.unwrap();

        assert_eq!(false, debounced.is_low().unwrap());
        assert_eq!(true, debounced.is_high().unwrap());

        // It is always safe to poll a stack-scoped Debouncer.
        unsafe { debouncer.poll() }.unwrap();

        assert_eq!(false, debounced.is_low().unwrap());
        assert_eq!(true, debounced.is_high().unwrap());

        // It is always safe to poll a stack-scoped Debouncer.
        unsafe { debouncer.poll() }.unwrap();

        assert_eq!(false, debounced.is_low().unwrap());
        assert_eq!(true, debounced.is_high().unwrap());

        // It is always safe to poll a stack-scoped Debouncer.
        unsafe { debouncer.poll() }.unwrap();

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        // It is always safe to deinit a stack-scopted Debouncer.
        let mut pin = unsafe { debouncer.deinit(debounced) }.unwrap();
        pin.done();
    }

    struct Cfg;
    impl Debounce for Cfg {
        type Storage = u8;
        const MAX_COUNT: u8 = 3;
        const INIT_HIGH: bool = false;
    }

    static SIMPLE_STATIC_TEST: Debouncer<pin::Mock, Cfg> = debouncer_uninit!();

    #[test]
    fn simple_static() {
        let expectations = [
            pin::Transaction::get(pin::State::High),
            pin::Transaction::get(pin::State::High),
            pin::Transaction::get(pin::State::High),
            pin::Transaction::get(pin::State::Low),
            pin::Transaction::get(pin::State::Low),
            pin::Transaction::get(pin::State::Low),
        ];

        let pin = pin::Mock::new(&expectations);

        // This is safe since this is the only test using this Debouncer.
        let debounced = unsafe { SIMPLE_STATIC_TEST.init(pin) }.expect("debounced pin");

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        unsafe { SIMPLE_STATIC_TEST.poll() }.unwrap();

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        unsafe { SIMPLE_STATIC_TEST.poll() }.unwrap();

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        unsafe { SIMPLE_STATIC_TEST.poll() }.unwrap();

        assert_eq!(false, debounced.is_low().unwrap());
        assert_eq!(true, debounced.is_high().unwrap());

        unsafe { SIMPLE_STATIC_TEST.poll() }.unwrap();

        assert_eq!(false, debounced.is_low().unwrap());
        assert_eq!(true, debounced.is_high().unwrap());

        unsafe { SIMPLE_STATIC_TEST.poll() }.unwrap();

        assert_eq!(false, debounced.is_low().unwrap());
        assert_eq!(true, debounced.is_high().unwrap());

        unsafe { SIMPLE_STATIC_TEST.poll() }.unwrap();

        assert_eq!(true, debounced.is_low().unwrap());
        assert_eq!(false, debounced.is_high().unwrap());

        // This is safe because this is the only test using this Debouncer.
        let mut pin = unsafe { SIMPLE_STATIC_TEST.deinit(debounced) }.unwrap();
        pin.done();
    }

    #[test]
    fn zero_sized_pin_type() {
        struct Pin;
        impl InputPin for Pin {
            type Error = core::convert::Infallible;
            fn is_high(&self) -> Result<bool, Self::Error> {
                Ok(true)
            }
            fn is_low(&self) -> Result<bool, Self::Error> {
                Ok(false)
            }
        }

        type MyDebouncer = Debouncer<Pin, default::ActiveLow>;

        assert_eq!(1, core::mem::size_of::<MyDebouncer>());
    }
}
