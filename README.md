edhex
-----

A hex editor that works vaguely like ed

Usage
=====
edhex <filename>

Differences from ed
===================
- Not every `ed` command is implemented
- The prompt `*` is printed by default.
- Instead of line number, use byte numbers.
- Byte numbers can be indicated in hex by starting with `0x` or decimal without.
- There's a width setting `W` for printing out.
- Byte numbers start from 0.
