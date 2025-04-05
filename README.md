# rs-file-server

This is a small webserver inspired by the python http.server module.

I mostly made this to share files on my local network.

The server is built to have as little dependencies as possible, so the only dependency is the flate crate which i use for gzip compression.
I may try to remove it later, depends if i feel like it.
Otherwise every other module comes from the standard rust libraries.
