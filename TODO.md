- BUG
  - ? past the beginning of the file causes a panic
  - Show context even on first print when opening file

- Handle things like "$-10"
- docs.rs documentation
- Implement printing only a screenful at a time (real ed doesn't bother to do this)
- Allow placing of breaks at random places in the bytes (to help indicate
  protocol things.
- Allow placing notes are locations in the bytes.
- Use cross platform colors so when compiled with mingw, still get terminal colors
  - vihex may handle this
