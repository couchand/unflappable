# unflappable

A batteries-included [embedded-hal] [`InputPin`] debouncer.

- [Quickstart](#quickstart)
- [Documentation](#documentation)
  - [API Docs](#api-docs)
  - [Minimum Supported Rust Version](#minimum-supported-rust-version)
- [Comparison to other debounce crates](#comparison-to-other-debounce-crates)
- [Contributing](#contributing)

## Quickstart

Add the following to your `Cargo.toml`:

```toml
[dependencies]
unflappable = "0.1"
```

Create an uninitialized [`Debouncer`] in static storage.

```rust
use unflappable::{debouncer_uninit, Debouncer, default::ActiveLow};
static DEBOUNCER: Debouncer<PinType, ActiveLow> = debouncer_uninit!();
```

Initialize the `Debouncer` with your input pin, returning the
[`Debounced`] pin.  Use this pin just as you would any other
[`InputPin`], such as passing ownership to another abstraction.

```rust
let debounced_pin = unsafe { DEBOUNCER.init(input_pin) }?;
```

Regularly poll the `Debouncer`, perhaps in an interrupt service routine.

```rust
unsafe {
    DEBOUNCER.poll()?;
}
```

## Documentation

### API Docs

API docs are hosted on docs.rs:

[API Documentation]

### Minimum Supported Rust Version

This crate makes use of trait bounds on a `const fn`, which is
currently unstable.  Therefore, we require use of the nightly
compiler.  When [rust-lang/rust#67792] stabilizes, we will
establish a MSRV policy.

## Comparison to other debounce crates

There are at least three debouncing crates targeting embedded Rust
development.  How does this one compare to the others?

- Crate: `unflappable`
  - Wraps `InputPin`: **Yes**
  - Can move wrapped pin: **Yes**
  - Algorithm: Integration-based by [Kuhn]
  - State overhead: `u8`
- Crate: [`debounced-pin`]
  - Wraps `InputPin`: **Yes**
  - Can move wrapped pin: No
  - Algorithm: Differentiation-based by [Greensted]
  - State overhead: `u8` + `enum`
- Crate: [`debouncr`]
  - Wraps `InputPin`: No
  - Can move wrapped pin: N/A
  - Algorithm: Differentiation-based by [Ganssle]
  - State overhead: `u8`
- Crate: [`debouncing`]
  - Wraps `InputPin`: No
  - Can move wrapped pin: N/A
  - Algorithm: Differentiation-based by [Hackaday]
  - State overhead: `u8` + dynamically-allocated `Vec`

## Contributing

I'm happy to see any and all contributions, including bug reports,
usability suggestions, patches, or angry yet well-intentioned rants.
You are encouraged to report issues to the official [issue tracker]
and send any questions or patches to the [mailing list].  Pull requests
to the GitHub mirror are also acceptable.

[embedded-hal]: https://github.com/rust-embedded/embedded-hal
[API Documentation]: https://docs.rs/unflappable
[rust-lang/rust#67792]: https://github.com/rust-lang/rust/issues/67792
[`Debouncer`]: https://docs.rs/unflappable/0.1.0/unflappable/struct.Debouncer.html
[`Debounced`]: https://docs.rs/unflappable/0.1.0/unflappable/struct.Debounced.html
[`InputPin`]: https://docs.rs/embedded-hal/0.2.4/embedded_hal/digital/v2/trait.InputPin.html
[issue tracker]: https://todo.sr.ht/~couch/unflappable
[mailing list]: https://lists.sr.ht/~couch/unflappable-dev
[Kuhn]: http://www.kennethkuhn.com/electronics/debounce.c
[`debounced-pin`]: https://github.com/Winseven4lyf/rust-debounced-pin
[Greensted]: http://www.labbookpages.co.uk/electronics/debounce.html#soft
[`debouncr`]: https://github.com/dbrgn/debouncr/
[Ganssle]: http://www.ganssle.com/debouncing-pt2.htm
[`debouncing`]: https://github.com/TyberiusPrime/debouncing
[Hackaday]: https://hackaday.com/2015/12/10/embed-with-elliot-debounce-your-noisy-buttons-part-ii/
