## Integrity checking & lock files

### Introduction

Let's say your module depends on remote module `https://some.url/a.ts`. When you
compile your module for the first time `a.ts` is retrieved, compiled and cached.
It will remain this way until you run your module on a new machine (say in
production) or reload the cache (through `deno cache --reload` for example). But
what happens if the content in the remote url `https://some.url/a.ts` is
changed? This could lead to your production module running with different
dependency code than your local module. Deno's solution to avoid this is to use
integrity checking and lock files.

### Caching and lock files

Deno can store and check subresource integrity for modules using a small JSON
file. Use the `--lock=lock.json` to enable and specify lock file checking. To
update or create a lock use `--lock=lock.json --lock-write`. The
`--lock=lock.json` tells Deno what the lock file to use is, while the
`--lock-write` is used to output dependency hashes to the lock file
(`--lock-write` must be used in conjunction with `--lock`).

A `lock.json` might look like this, storing a hash of the file against the
dependency:

```json
{
  "https://deno.land/std@$STD_VERSION/textproto/mod.ts": "3118d7a42c03c242c5a49c2ad91c8396110e14acca1324e7aaefd31a999b71a4",
  "https://deno.land/std@$STD_VERSION/io/util.ts": "ae133d310a0fdcf298cea7bc09a599c49acb616d34e148e263bcb02976f80dee",
  "https://deno.land/std@$STD_VERSION/async/delay.ts": "35957d585a6e3dd87706858fb1d6b551cb278271b03f52c5a2cb70e65e00c26a",
   ...
}
```

A typical workflow will look like this:

**src/deps.ts**

```ts
// Add a new dependency to "src/deps.ts", used somewhere else.
export { xyz } from "https://unpkg.com/xyz-lib@v0.9.0/lib.ts";
```

Then:

```shell
# Create/update the lock file "lock.json".
deno cache --lock=lock.json --lock-write src/deps.ts

# Include it when committing to source control.
git add -u lock.json
git commit -m "feat: Add support for xyz using xyz-lib"
git push
```

Collaborator on another machine -- in a freshly cloned project tree:

```shell
# Download the project's dependencies into the machine's cache, integrity
# checking each resource.
deno cache --reload --lock=lock.json src/deps.ts

# Done! You can proceed safely.
deno test --allow-read src
```

### Runtime verification

Like caching above, you can also use the `--lock=lock.json` option during use of
the `deno run` sub command, validating the integrity of any locked modules
during the run. Remember that this only validates against dependencies
previously added to the `lock.json` file. New dependencies will be cached but
not validated.

You can take this a step further as well by using the `--cached-only` flag to
require that remote dependencies are already cached.

```shell
deno run --lock=lock.json --cached-only mod.ts
```

This will fail if there are any dependencies in the dependency tree for mod.ts
which are not yet cached.

<!-- TODO - Add detail on dynamic imports -->
