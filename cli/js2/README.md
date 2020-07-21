# js2

This directory contains Deno runtime code written in plain JavaScript.

Each file is a plain, old **script**, not ES modules. The reason is that
snapshotting ES modules is much harder, especially if one needs to manipulate
global scope (like in case of Deno).

Each file is prefixed with a number, telling in which order scripts should be
loaded into V8 isolate. This is temporary solution and we're striving not to
require specific order (though it's not 100% obvious if that's feasible).
