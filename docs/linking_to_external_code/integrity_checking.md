## Integrity checking & lock files

Deno can store and check subresource integrity for modules using a small JSON
file. Use the `--lock=lock.json` to enable and specify lock file checking. To
update or create a lock use `--lock=lock.json --lock-write`.

A typical workflow will look like this:

```ts
// Add a new dependency to "src/deps.ts", used somewhere else.
export { xyz } from "https://unpkg.com/xyz-lib@v0.9.0/lib.ts";
```

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
deno cache -r --lock=lock.json src/deps.ts

# Done! You can proceed safely.
deno test --allow-read src
```
