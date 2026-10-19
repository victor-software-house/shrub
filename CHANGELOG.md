# Changelog

## shrub 0.0.4

- The shared verify runs verctl's version guard: ctl-core's Rust tasks at 0.6.27 and verctl at 0.5.3, so a manifest version changes only on the Version PR ([#26](https://github.com/victor-software-house/shrub/pull/26)).

## shrub 0.0.3

- A brace sequence with an increment too large for a 64-bit integer, such as `echo {1..3..99999999999999999999}`, no longer panics and stays literal text, as in bash, and a very large increment that does fit no longer overflows or loops forever while the sequence is expanded ([#23](https://github.com/victor-software-house/shrub/pull/23)).
- Every fallible or panicking public function now documents when it errors or panics ([#14](https://github.com/victor-software-house/shrub/pull/14)).

## shrub 0.0.2

- Arithmetic takes `++` or `--` before anything but a variable as two signs, as bash does, so `$((++0))` is 0 instead of a parse error ([#9](https://github.com/victor-software-house/shrub/pull/9)).
- Every library crate has a docs.rs landing page, and docs.rs builds each crate with all features enabled ([#15](https://github.com/victor-software-house/shrub/pull/15)).
- First release of shrub, a hard fork of brush: every crate is published under the shrub name ([#2](https://github.com/victor-software-house/shrub/pull/2)).
- The `shrub` crate now holds the only `shrub` binary and passes through every shrub-shell feature, so install with `cargo install shrub`. shrub-shell is now a library; its command line and entry point moved to `shrub` ([#11](https://github.com/victor-software-house/shrub/pull/11)).

