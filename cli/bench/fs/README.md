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

### view report

```bash
deno run --allow-net=127.0.0.1:9000 serve.jsx
# View rendered report at http://127.0.0.1:9000/
```
