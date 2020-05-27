# std/browser

A collection of modules to help create code that is portable between Deno and
browsers.

## std/browser/deno_shim.ts

A module that provides a shim of the `Deno` namespace for browsers, providing as
much of the unique Deno APIs that can be easily supported on the browser or
aiming to provide a non-operation capability.

When compiling or bundling code in Deno that is also intended to be used in a
browser, it is sometimes difficult to properly cater for the lack of the `Deno`
namespace, especially when using strongly typed code which will emit errors.
This module intends to solve that.

### Usage

This module is designed to just be imported by a script. When the module loads,
it will detect if the `Deno` namespace is present in the global scope and if not
it will add the the shim for `Deno`. This should allow most Deno code to access
the `Deno` global object without issue, as well as perform some `Deno` API
functions in a browser in a "virtual way".

Specifically this is designed to be used when creating a bundle via
`deno bundle` that uses `Deno` APIs that you want to run in the web. For
example, you would want to put this in the root file of your bundle:

```ts
import "https://deno.land/std/browsers/deno_shim.ts";
```

And if you were to generate a bundle, the shim would be included in your code,
and when that bundle is loaded in the browser, accessing most `Deno` APIs will
not cause your application to throw.

Generally, if you plan for your code to work both in a browser and Deno, you
should try to detect the environment you are running in and avoid using `Deno`
APIs. One easy way to do that is that `Deno.build.target` will be set to
`"browser"` when the shim is loaded.

#### Unstable APIs

If you want to shim Deno's unstable APIs, like when using the `--unstable` flag
with the Deno CLI, there is an export of the module which can do this for you.
Import the `unstable()` function and await it, which will add the unstable APIs
to the `Deno` namespace.

```ts
import { unstable } from "https://deno.land/std/browsers/deno_shim.ts";

await unstable();
```

If your target browser does not support top-level-await yet, you will need to
avoid using it, even when bundling, by using an IIFE:

```ts
import { unstable } from "https://deno.land/std/browsers/deno_shim.ts";

(async () => {
  await unstable();
})();
```

### Virtual File System and Resources

Most of the file system IO operations have been stubbed out in the shim, where
there will behave similar to the way they do when running within Deno. You can
open, read, write, and close in memory virtual files. At this point, there is no
sense of file structure, files are simply matched based on their path string,
and there is no file content in any file that is create or opened at load time.
In order to put content in files, the file would have to be opened with a create
flag, and written to. Files can be opened multiple times though, and each
resource ID can have its own position in the file (and own permissions for
reading and writing).

When closing a file, it will stay in memory, until the page is reloaded. In
order to free up closed files in memory, the `deno_shim.ts` module exports a
function named `purgeResources()` which will dereference the closed virtual
files so their data can be garbage collected by the browser.

### Unimplemented APIs

There are several `Deno` APIs where there is an expected return value, of which
there is not a logical non-operation that can be performed, in these cases,
these APIs will throw with an `Error` saying the feature isn't implemented:

- `Deno.readLinkSync`
- `Deno.readLink`
- `Deno.lstat`
- `Deno.lstatSync`
- `Deno.stat`
- `Deno.statSync`
- `Deno.listen`
- `Deno.listenTls`
- `Deno.connect`
- `Deno.connectTls`
- `Deno.Process`
- `Deno.run`
- `Deno.inspect`

And there are some unstable:

- `Deno.openPlugin`
- `Deno.formatDiagnostics`
- `Deno.trasnpileOnly`
- `Deno.compile`
- `Deno.bundle`
- `Deno.applySourceMap`
- `Deno.SignalStream`
- `Deno.signal`
- `Deno.listenDatagram`
- `Deno.startTls`
