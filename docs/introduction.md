# Introduction

Deno is a JavaScript/TypeScript runtime with secure defaults and a great
developer experience.

It's built on V8, Rust, and Tokio.

## Feature highlights

- Secure by default. No file, network, or environment access (unless explicitly
  enabled).
- Supports TypeScript out of the box.
- Ships a single executable (`deno`).
- Has built-in utilities like a dependency inspector (`deno info`) and a code
  formatter (`deno fmt`).
- Has
  [a set of reviewed (audited) standard
  modules](https://github.com/denoland/deno_std) that are guaranteed
  to work with Deno.
- Scripts can be bundled into a single JavaScript file.

## Philosophy

Deno aims to be a productive and secure scripting environment for the modern
programmer.

Deno will always be distributed as a single executable. Given a URL to a Deno
program, it is runnable with nothing more than
[the ~15 megabyte zipped executable](https://github.com/denoland/deno/releases).
Deno explicitly takes on the role of both runtime and package manager. It uses a
standard browser-compatible protocol for loading modules: URLs.

Among other things, Deno is a great replacement for utility scripts that may
have been historically written with Bash or Python.

## Goals

- Only ship a single executable (`deno`).
- Provide secure defaults.
  - Unless specifically allowed, scripts can't access files, the environment, or
    the network.
- Be browser-compatible.
  - The subset of Deno programs which are written completely in JavaScript and
    do not use the global `Deno` namespace (or feature test for it), ought to
    also be able to be run in a modern web browser without change.
- Provide built-in tooling to improve developer experience.
  - E.g. unit testing, code formatting, and linting.
- Not leak V8 concepts into user land.
- Serve HTTP efficiently.

## Comparison to Node.js

- Deno does not use `npm`.
  - It uses modules referenced as URLs or file paths.
- Deno does not use `package.json` in its module resolution algorithm.
- All async actions in Deno return a promise. Thus Deno provides different APIs
  than Node.
- Deno requires explicit permissions for file, network, and environment access.
- Deno always dies on uncaught errors.
- Uses "ES Modules" and does not support `require()`. Third party modules are
  imported via URLs:

  ```javascript
  import * as log from "https://deno.land/std@$STD_VERSION/log/mod.ts";
  ```

## Other key behaviors

- Remote code is fetched and cached on first execution, and never updated until
  the code is run with the `--reload` flag. (So, this will still work on an
  airplane.)
- Modules/files loaded from remote URLs are intended to be immutable and
  cacheable.
