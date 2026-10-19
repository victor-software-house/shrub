---
shrub: patch
---

A brace sequence with an increment too large for a 64-bit integer, such as `echo {1..3..99999999999999999999}`, no longer panics and stays literal text, as in bash.
