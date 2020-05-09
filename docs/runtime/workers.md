## Workers

Deno supports
[`Web Worker API`](https://developer.mozilla.org/en-US/docs/Web/API/Worker/Worker).

Workers can be used to run code on multiple threads. Each instance of `Worker`
is run on a separate thread, dedicated only to that worker.

Currently Deno supports only `module` type workers; thus it's essential to pass
`type: "module"` option when creating new worker:

```ts
// Good
new Worker("./worker.js", { type: "module" });

// Bad
new Worker("./worker.js");
new Worker("./worker.js", { type: "classic" });
```

### Using Deno in worker

> This is an unstable Deno feature. Learn more about
> [unstable features](./stability.md).

By default `Deno` namespace is not available in worker scope.

To add `Deno` namespace pass `deno: true` option when creating new worker:

```ts
// main.js
const worker = new Worker("./worker.js", { type: "module", deno: true });
worker.postMessage({ filename: "./log.txt" });

// worker.js
self.onmessage = async (e) => {
  const { filename } = e.data;
  const text = await Deno.readTextFile(filename);
  console.log(text);
  self.close();
};

// log.txt
hello world
```

```shell
$ deno run --allow-read --unstable main.js
hello world
```

When `Deno` namespace is available in worker scope; the worker inherits parent
process permissions (the ones specified using `--allow-*` flags).

We intend to make permissions configurable for workers.
