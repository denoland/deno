# Runtime

Documentation for all runtime functions (Web APIs + `Deno` global) can be found
on
[`doc.deno.land`](https://doc.deno.land/https/github.com/denoland/deno/releases/latest/download/lib.deno.d.ts).

## Web Platform APIs

For APIs where a web standard already exists, like `fetch` for HTTP requests,
Deno uses these rather than inventing a new proprietary API.

For more details, view the chapter on
[Web Platform APIs](./runtime/web_platform_apis.md).

## `Deno` global

All APIs that are not web standard are contained in the global `Deno` namespace.
It has the APIs for reading from files, opening TCP sockets, and executing
subprocesses, etc.

The TypeScript definitions for the Deno namespaces can be found in the
[`lib.deno.ns.d.ts`](https://github.com/denoland/deno/blob/$CLI_VERSION/cli/dts/lib.deno.ns.d.ts)
file.

The documentation for all of the Deno specific APIs can be found on
[doc.deno.land](https://doc.deno.land/https/raw.githubusercontent.com/denoland/deno/master/cli/dts/lib.deno.ns.d.ts).
