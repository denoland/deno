## `fs` benchmarks

### adding new benchmarks

```js
const copyFileSync = getFunction("copyFileSync");
bench(() => copyFileSync("test", "test2"));

// For functions with side-effects, clean up after `bench` like so:
const removeSync = getFunction("removeSync");
removeSync("test2");
```

### running

```bash
deno run -A --unstable run.mjs
node run.js
```
