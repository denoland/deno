# tsc

This directory contains the code for the typescript compiler snapshot

There is currently A LOT of overlap between this code and the runtime snapshot
code in cli/rt.

This is intentionally ugly because there should be no overlap.

This directory ultimately should contain just typescript.js and a smallish
CompilerHost.
