edhex
=====

A hex editor that works vaguely like ed

Usage
-----
edhex <filename>

Differences from ed
-------------------
- Not every `ed` command is implemented
- The prompt `*` is printed by default.
- Instead of line number, use byte numbers.
- There's a width setting `W` for printing out.
- Byte numbers start from 0.
- a, b, c, d, e, f can't be commands because they could be numbers.
