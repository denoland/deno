# tsc

This directory contains the typescript compiler and a small compiler host for
the runtime snapshot.

## How to upgrade TypeScript.

The files in this directory are mostly from the TypeScript repository. We
currently (unfortunately) have a rather manual process for upgrading TypeScript.
It works like this currently:

1. Checkout denoland/TypeScript repo in a separate directory.
1. Add Microsoft/TypeScript as a remote and fetch its latest tags
1. Checkout a new branch based on this tag.
1. Cherry pick the custom commit we made in a previous release to the new one.
1. This commit has a "deno.ts" file in it. Read the instructions in it.
1. Copy typescript.js into Deno repo.
1. Copy d.ts files into dts directory.
1. Review the copied files, removing and reverting what's necessary

So that might look something like this:

```
git clone https://github.com/denoland/TypeScript.git
cd typescript
git remote add upstream https://github.com/Microsoft/TypeScript
git fetch upstream
git checkout v3.9.7
git checkout -b branch_v3.9.7
git cherry pick <previous-release-branch-commit-we-did>
npm install
npx hereby
rsync built/local/typescript.js ~/src/deno/cli/tsc/00_typescript.js
rsync --exclude=protocol.d.ts --exclude=tsserverlibrary.d.ts --exclude=typescriptServices.d.ts built/local/*.d.ts ~/src/deno/cli/tsc/dts/
```

## Typescript-Go Integration

Currently only integrated with deno check, though in the future it will also be
integrated with our LSP implementation.

In the CLI, we have a small abstraction over the tsc backend in
[cli/tsc/mod.rs](./mod.rs). Along with some shared types and functionality, the
main piece is the `exec` function, which takes a "request" to be served by the
typescript compiler and returns the result. This now has two different "backend"
which can serve the request – the current tsc, which runs in an isolate and
communicates via ops, and typescript-go which runs in a subprocess and uses IPC.

From a high level, the way the tsgo backend works is that we download a
typescript-go binary from
[github releases](https://github.com/denoland/typescript-go/releases) into the
deno cache dir. To actually interface with tsgo, we spawn it in a subprocess and
write messages over stdin/stdout (similar to the Language Server Protocol). The
format is a mixture of binary data (for the header and other protocol level
details) followed by json encoded values for RPC calls. The rust implementation
of the IPC protocol is in the
[deno_typescript_go_client_rust crate](../libs/typescript_go_client/src/lib.rs).

We currently maintain a
[fork of typescript-go](https://github.com/denoland/typescript-go) with the
following changes:

- Special handling of the global symbol tables to account for the fact that we
  have two slightly different sets of globals: one for node contexts (in npm
  packages), and one for deno contexts. At this point, the main difference is
  the type returned by `setTimeout`. With node globals `setTimeout` returns an
  object, and with deno globals it returns a number (just like the web
  standard).
- Symbol table logic to prevent @types/node from creating type errors by
  introducing incompatible definitions for globals
- Additional hooks to allow us to provide our own resolution, determine whether
  a file is esm/cjs, etc.
- Additional APIs exposed from the IPC server
- Support for deno's custom libs (`deno.window`, `deno.worker`, etc)
