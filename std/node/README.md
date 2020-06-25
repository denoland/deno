# Deno Node compatibility

This module is meant to have a compatibility layer for the
[NodeJS standard library](https://nodejs.org/docs/latest-v12.x/api/).

**Warning**: Any function of this module should not be referred anywhere in the
deno standard library as it's a compatibility module.

## Supported Builtins

- [ ] assert
- [x] buffer
- [ ] child_process
- [ ] cluster
- [ ] console
- [ ] crypto
- [ ] dgram
- [ ] dns
- [x] events
- [x] fs _partly_
- [ ] http
- [ ] http2
- [ ] https
- [x] module
- [ ] net
- [x] os _partly_
- [x] path
- [ ] perf_hooks
- [x] process _partly_
- [x] querystring
- [ ] readline
- [ ] repl
- [ ] stream
- [ ] string_decoder
- [ ] sys
- [x] timers
- [ ] tls
- [ ] tty
- [ ] url
- [x] util _partly_
- [ ] ~~v8~~ _can't implement_
- [ ] vm
- [ ] worker_threads
- [ ] zlib

* [x] node globals _partly_

### Deprecated

These builtins are deprecated in NodeJS v13 and will probably not be polyfilled:

- constants
- domain
- freelist
- punycode

### Experimental

These builtins are experimental in NodeJS v13 and will not be polyfilled until
they are stable:

- async_hooks
- inspector
- policies
- report
- trace_events
- wasi

## CommonJS Module Loading

`createRequire(...)` is provided to create a `require` function for loading CJS
modules. It also sets supported globals.

```ts
import { createRequire } from "https://deno.land/std/node/module.ts";

const require = createRequire(import.meta.url);
// Loads native module polyfill.
const path = require("path");
// Loads extensionless module.
const cjsModule = require("./my_mod");
// Visits node_modules.
const leftPad = require("left-pad");
```
