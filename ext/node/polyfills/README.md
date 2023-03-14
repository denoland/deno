# Deno Node.js compatibility

This module is meant to have a compatibility layer for the
[Node.js standard library](https://nodejs.org/docs/latest/api/).

**Warning**: Any function of this module should not be referred anywhere in the
Deno standard library as it's a compatibility module.

## Supported modules

- [x] assert
- [x] assert/strict _partly_
- [x] async_hooks _partly_
- [x] buffer
- [x] child_process _partly_
- [x] cluster _partly_
- [x] console _partly_
- [x] constants _partly_
- [x] crypto _partly_
- [x] dgram _partly_
- [x] diagnostics_channel _partly_
- [x] dns _partly_
- [x] events
- [x] fs _partly_
- [x] fs/promises _partly_
- [x] http _partly_
- [ ] http2
- [x] https _partly_
- [x] inspector _partly_
- [x] module
- [x] net
- [x] os _partly_
- [x] path
- [x] path/posix
- [x] path/win32
- [x] perf_hooks
- [x] process _partly_
- [x] punycode
- [x] querystring
- [x] readline
- [x] repl _partly_
- [x] stream
- [x] stream/promises
- [x] stream/web _partly_
- [x] string_decoder
- [x] sys
- [x] timers
- [x] timers/promises
- [ ] tls
- [ ] trace_events
- [x] tty _partly_
- [x] url
- [x] util _partly_
- [x] util/types _partly_
- [ ] v8
- [x] vm _partly_
- [x] wasi
- [ ] webcrypto
- [x] worker_threads
- [ ] zlib

* [x] node globals _partly_

### Deprecated

These modules are deprecated in Node.js and will probably not be polyfilled:

- domain
- freelist

### Experimental

These modules are experimental in Node.js and will not be polyfilled until they
are stable:

- diagnostics_channel
- async_hooks
- policies
- trace_events
- wasi
- webcrypto

## CommonJS modules loading

`createRequire(...)` is provided to create a `require` function for loading CJS
modules. It also sets supported globals.

```ts
import { createRequire } from "https://deno.land/std@$STD_VERSION/node/module.ts";

const require = createRequire(import.meta.url);
// Loads native module polyfill.
const path = require("path");
// Loads extensionless module.
const cjsModule = require("./my_mod");
// Visits node_modules.
const leftPad = require("left-pad");
```

## Contributing

### Setting up the test runner

This library contains automated tests pulled directly from the Node.js repo in
order ensure compatibility.

Setting up the test runner is as simple as running the `node/_tools/setup.ts`
file, this will pull the configured tests in and then add them to the test
workflow.

```zsh
$ deno task node:setup
```

You can additionally pass the `-y`/`-n` flag to use test cache or generating
tests from scratch instead of being prompted at the moment of running it.

```zsh
# Will use downloaded tests instead of prompting user
$ deno run --allow-read --allow-net --allow-write node/_tools/setup.ts -y
# Will not prompt but will download and extract the tests directly
$ deno run --allow-read --allow-net --allow-write node/_tools/setup.ts -n
```

To run the tests you have set up, do the following:

```zsh
$ deno test --allow-read --allow-run node/_tools/test.ts
```

If you want to run specific Node.js test files, you can use the following
command

```shellsession
$ deno test -A node/_tools/test.ts -- <pattern-to-match>
```

For example, if you want to run only
`node/_tools/test/parallel/test-event-emitter-check-listener-leaks.js`, you can
use:

```shellsession
$ deno test -A node/_tools/test.ts -- test-event-emitter-check-listener-leaks.js
```

If you want to run all test files which contains `event-emitter` in filename,
then you can use:

```shellsession
$ deno test -A node/_tools/test.ts -- event-emitter
```

The test should be passing with the latest deno, so if the test fails, try the
following:

- `$ deno upgrade`
- `$ git submodule update --init`
- Use
  [`--unstable` flag](https://deno.land/manual@v1.15.3/runtime/stability#standard-modules)

To enable new tests, simply add a new entry inside `node/_tools/config.json`
under the `tests` property. The structure this entries must have has to resemble
a path inside `https://github.com/nodejs/node/tree/main/test`.

Adding a new entry under the `ignore` option will indicate the test runner that
it should not regenerate that file from scratch the next time the setup is run,
this is specially useful to keep track of files that have been manually edited
to pass certain tests. However, avoid doing such manual changes to the test
files, since that may cover up inconsistencies between the node library and
actual node behavior.

### Working with child processes ? Use `DENO_NODE_COMPAT_URL`

When working with `child_process` modules, you will have to run tests pulled
from Node.js. These tests usually spawn deno child processes via the use of
`process.execPath`. The `deno` executable will use its own embedded version of
std modules, then you may get the impression your code is not really working as
it should.

To prevent this, set `DENO_NODE_COMPAT_URL` with the absolute path to your
`deno_std` repo, ending with a trailing slash:

```
export DENO_NODE_COMPAT_URL=$PWD/
# or
export DENO_NODE_COMPAT_URL=file:///path/to/deno_std/dir/
```

Then, `deno` will use your local copy of `deno_std` instead of latest version.

### Best practices

When converting from promise-based to callback-based APIs, the most obvious way
is like this:

```ts, ignore
promise.then((value) => callback(null, value)).catch(callback);
```

This has a subtle bug - if the callback throws an error, the catch statement
will also catch _that_ error, and the callback will be called twice. The correct
way to do it is like this:

```ts, ignore
promise.then((value) => callback(null, value), callback);
```

The second parameter of `then` can also be used to catch errors, but only errors
from the existing promise, not the new one created by the callback.

If the Deno equivalent is actually synchronous, there's a similar problem with
try/catch statements:

```ts, ignore
try {
  const value = process();
  callback(null, value);
} catch (err) {
  callback(err);
}
```

Since the callback is called within the `try` block, any errors from it will be
caught and call the callback again.

The correct way to do it is like this:

```ts, ignore
let err, value;
try {
  value = process();
} catch (e) {
  err = e;
}
if (err) {
  callback(err); // Make sure arguments.length === 1
} else {
  callback(null, value);
}
```

It's not as clean, but prevents the callback being called twice.

### Remaining Tests

Node compatibility can be measured by how many native Node tests pass. If you'd
like to know what you can work on, check out the list of Node tests remaining
[here](_tools/TODO.md).
