---
shrub: minor
---

shrub formats and stores times with `jiff` instead of `chrono`, which RUSTSEC-2026-0543 marks unmaintained.
This breaks `shrub-core`'s API: `shrub_core::history::ItemTimestamp` is now `jiff::Timestamp`, and `ErrorKind` gains `TimeFormatError`.
`shrub_core::time_format::format_local` formats a timestamp in the system time zone, as the prompt's `\D{format}` and `HISTTIMEFORMAT` do.
Their conversions still print as in the POSIX locale.
An unknown conversion now prints as written, as glibc's `strftime` leaves it, where it used to stop the shell with a panic.
`%f`, which `strftime` lacks, now prints the fraction of a second without trailing zeros, where `chrono` padded it to nine digits.
