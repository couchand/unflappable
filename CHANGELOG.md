# Changelog

All notable changes to this project will be documented in this file.

The format of this file is based on the recommendations in
[Keep a Changelog].
Like most crates in the Rust ecosystem this project adheres to
[Semantic Versioning].

## [Unreleased]

- *nothing yet*

## [v0.2.0] - 2023-04-18 ([Log][v0.2.0-log])

### Changed

- The feature flag `#[const_fn]` has been removed (thanks Dave O!).
- The MSRV has been set to 1.61.
- Usage documentation has been improved.

## [v0.1.0] - 2020-08-12 ([Log][v0.1.0-log])

- Initial release of `unflappable`.
- Wraps the `InputPin`, returning a moveable `impl InputPin`.
- Supports seamless static storage for use in an interrupt service
  routine.

[Keep a Changelog]: https://keepachangelog.com/en/1.1.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
[Unreleased]: https://git.sr.ht/~couch/unflappable/log
[v0.2.0]: https://git.sr.ht/~couch/unflappable/refs/v0.2.0
[v0.2.0-log]: https://git.sr.ht/~couch/unflappable/log/v0.2.0
[v0.1.0]: https://git.sr.ht/~couch/unflappable/refs/v0.1.0
[v0.1.0-log]: https://git.sr.ht/~couch/unflappable/log/v0.1.0
