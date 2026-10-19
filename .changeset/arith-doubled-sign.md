---
shrub: patch
---

Arithmetic takes `++` or `--` before anything but a variable as two signs, as bash does, so `$((++0))` is 0 instead of a parse error.
