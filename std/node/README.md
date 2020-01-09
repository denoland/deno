# Deno Node compatibility

This module is meant to have a compatibility layer for the
[NodeJS standard library](https://nodejs.org/docs/latest-v12.x/api/).

**Warning**: Any function of this module should not be referred anywhere in the
deno standard library as it's a compatiblity module.

## Supported Builtins

- [ ] assert
- [ ] ~~async_hooks~~ _experimental_
- [ ] buffer
- [ ] child_process
- [ ] cluster
- [ ] console
- [ ] ~~constants~~ _deprecated_
- [ ] crypto _deno needs this first_
- [ ] dgram
- [ ] dns
- [ ] ~~domain~~ _deprecated_
- [ ] events
- [ ] ~~freelist~~ _deprecated_
- [ ] fs _partly_
- [ ] http
- [ ] http2
- [ ] https
- [ ] ~~inspector~~ _experimental_
- [x] module
- [ ] net
- [ ] os
- [x] path
- [ ] perf_hooks
- [ ] ~~policies~~ _experimental_
- [ ] process
- [ ] ~~punycode~~ _deprecated_
- [ ] querystring
- [ ] readline
- [ ] repl
- [ ] ~~report~~ _experimental_
- [ ] stream
- [ ] string_decoder
- [ ] sys
- [x] timers
- [ ] tls
- [ ] ~~trace_events~~ _experimental_
- [ ] tty
- [ ] url
- [ ] util _partly_
- [ ] ~~v8~~ _can't implement_
- [ ] vm
- [ ] ~~wasi~~ _experimental_
- [ ] worker_threads
- [ ] zlib

* [ ] node globals

## CommonJS Module Loading

`createRequire(...)` is provided to create a `require` function for loading CJS
modules.

```ts
import { createRequire } from "https://deno.land/std/node/module.ts";

const require_ = createRequire(import.meta.url);
// Loads native module polyfill.
const path = require_("path");
// Loads extensionless module.
const cjsModule = require_("./my_mod");
// Visits node_modules.
const leftPad = require_("left-pad");
```
