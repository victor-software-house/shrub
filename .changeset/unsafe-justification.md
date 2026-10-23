---
shrub: patch
---

Each module with unsafe code now says why safe code cannot do its job, and each `unsafe` block's `SAFETY:` comment names the callee's preconditions and how they are met.
A child that takes the foreground no longer calls `std::io::stdin()` between the fork and `exec`, whose first call allocates; it calls `tcsetpgrp` on descriptor 0, and still ignores its failure.
A child that leads a session builds its `setsid` and `TIOCSCTTY` errors from the errno instead of allocating a message, so the spawn error is the OS error alone.
`sys::fd::try_get_file_for_open_fd` and `try_iter_open_fds` duplicate a descriptor number with `fcntl(F_DUPFD_CLOEXEC)` instead of borrowing a descriptor that may have closed.
