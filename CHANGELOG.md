# Changelog

## shrub 0.0.2

- Arithmetic takes `++` or `--` before anything but a variable as two signs, as bash does, so `$((++0))` is 0 instead of a parse error ([#9](https://github.com/victor-software-house/shrub/pull/9)).
- Every library crate has a docs.rs landing page, and docs.rs builds each crate with all features enabled ([#15](https://github.com/victor-software-house/shrub/pull/15)).
- First release of shrub, a hard fork of brush: every crate is published under the shrub name ([#2](https://github.com/victor-software-house/shrub/pull/2)).
- The `shrub` crate now holds the only `shrub` binary and passes through every shrub-shell feature, so install with `cargo install shrub`. shrub-shell is now a library; its command line and entry point moved to `shrub` ([#11](https://github.com/victor-software-house/shrub/pull/11)).

