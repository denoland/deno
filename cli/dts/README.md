# How to upgrade TypeScript.

The files in this directory are mostly from the TypeScript repository. We
currently (unfortunately) have a rather manual process for upgrading TypeScript.
It works like this currently:

1. Checkout typescript repo in a seperate directory.
2. Copy typescript.js into Deno repo
3. Copy d.ts files into dts directory

So that might look something like this:

```
git clone https://github.com/microsoft/TypeScript.git
cd typescript
git checkout v3.9.7
cp lib/typescript.js ~/src/deno/cli/tsc/00_typescript.js
cp lib/*.d.ts ~/src/deno/cli/dts/
```
