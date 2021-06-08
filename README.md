edhex
=====

A hex editor that works vaguely like ed

![screenshot](screenshot.png?raw=true)

Usage
-----

    edhex <filename>
    echo '0,$' |edhex -q <filename>

Differences from ed
-------------------
- Not every `ed` command is implemented
- The prompt `*` is printed by default.
- Uses byte numbers instead of line numbers.
- There's a width setting `W` for printing out.
- Byte numbers start from 0.
- a, b, c, d, e, f can't be commands because they could be numbers.
