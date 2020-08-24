## Handle OS Signals

> This program makes use of an unstable Deno feature. Learn more about
> [unstable features](../runtime/stability.md).

[API Reference](https://doc.deno.land/https/raw.githubusercontent.com/denoland/deno/master/cli/dts/lib.deno.unstable.d.ts#Deno.signal)

You can use `Deno.signal()` function for handling OS signals:

```ts
for await (const _ of Deno.signal(Deno.Signal.SIGINT)) {
  console.log("interrupted!");
}
```

`Deno.signal()` also works as a promise:

```ts
await Deno.signal(Deno.Signal.SIGINT);
console.log("interrupted!");
```

If you want to stop watching the signal, you can use `dispose()` method of the
signal object:

```ts
const sig = Deno.signal(Deno.Signal.SIGINT);
setTimeout(() => {
  sig.dispose();
}, 5000);

for await (const _ of sig) {
  console.log("interrupted");
}
```

The above for-await loop exits after 5 seconds when `sig.dispose()` is called.
