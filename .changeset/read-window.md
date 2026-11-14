---
shrub: minor
---

A program that embeds shrub can serve a file from a later first byte, such as an output whose oldest bytes it dropped in place.
`shrub_core::window::ReadWindow` names the first byte to read of the files it answers for, and the embedder sets it in `RuntimeOptions::read_window`.
Three experimental builtins ask it first, each behind its own feature of `shrub-experimental-builtins`, off by default: `cat` (`builtin.cat`), `head` (`builtin.head`) and `tail` (`builtin.tail`).
Each runs the program of its name in `PATH`; a file the window answers for is opened at that byte and given to the program on its standard input, so the program's options and output stay, and `head` and `tail` print a header per file when given several.
