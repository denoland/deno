# Handle OS Signals

> This program makes use of an unstable Deno feature. Learn more about
> [unstable features](../runtime/stability.md).

## Concepts

- Use the `--unstable` flag to access new or unstable features in Deno.
- [Deno.signal](https://doc.deno.land/builtin/unstable#Deno.signal) can be used
  to capture and monitor OS signals.
- Use the `dispose()` function of the Deno.signal
  [SignalStream](https://doc.deno.land/builtin/unstable#Deno.SignalStream) to
  stop watching the signal.

## Async iterator example

You can use `Deno.signal()` function for handling OS signals:

```ts
/**
 * async-iterator-signal.ts
 */
console.log("Press Ctrl-C to trigger a SIGINT signal");
for await (const _ of Deno.signal(Deno.Signal.SIGINT)) {
  console.log("interrupted!");
  Deno.exit();
}
```

Run with:

```shell
deno run --unstable async-iterator-signal.ts
```

## Promise based example

`Deno.signal()` also works as a promise:

```ts
/**
 * promise-signal.ts
 */
console.log("Press Ctrl-C to trigger a SIGINT signal");
await Deno.signal(Deno.Signal.SIGINT);
console.log("interrupted!");
Deno.exit();
```

Run with:

```shell
deno run --unstable promise-signal.ts
```

## Stop watching signals

If you want to stop watching the signal, you can use `dispose()` method of the
signal object:

```ts
/**
 * dispose-signal.ts
 */
const sig = Deno.signal(Deno.Signal.SIGINT);
setTimeout(() => {
  sig.dispose();
  console.log("No longer watching SIGINT signal");
}, 5000);

console.log("Watching SIGINT signals");
for await (const _ of sig) {
  console.log("interrupted");
}
```

Run with:

```shell
deno run --unstable dispose-signal.ts
```

The above for-await loop exits after 5 seconds when `sig.dispose()` is called.
