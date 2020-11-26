# async

async is a module to provide help with asynchronous tasks.

# Usage

The following functions and class are exposed in `mod.ts`:

## deferred

Create a Promise with the `reject` and `resolve` functions.

```typescript
import { deferred } from "https://deno.land/std/async/mod.ts";

const p = deferred<number>();
// ...
p.resolve(42);
```

## delay

Resolve a Promise after a given amount of milliseconds.

```typescript
import { delay } from "https://deno.land/std/async/mod.ts";

// ...
const delayedPromise = delay(100);
const result = await delayedPromise;
// ...
```

## MuxAsyncIterator

The MuxAsyncIterator class multiplexes multiple async iterators into a single
stream.

The class makes an assumption that the final result (the value returned and not
yielded from the iterator) does not matter. If there is any result, it is
discarded.

```typescript
import { MuxAsyncIterator } from "https://deno.land/std/async/mod.ts";

async function* gen123(): AsyncIterableIterator<number> {
  yield 1;
  yield 2;
  yield 3;
}

async function* gen456(): AsyncIterableIterator<number> {
  yield 4;
  yield 5;
  yield 6;
}

const mux = new MuxAsyncIterator<number>();
mux.add(gen123());
mux.add(gen456());
for await (const value of mux) {
  // ...
}
// ..
```

## pooledMap

Transform values from an (async) iterable into another async iterable. The
transforms are done concurrently, with a max concurrency defined by the
poolLimit.

```typescript
import { pooledMap } from "https://deno.land/std/async/mod.ts";

const results = pooledMap(
  2,
  [1, 2, 3],
  (i) => new Promise((r) => setTimeout(() => r(i), 1000)),
);

for await (const value of results) {
  // ...
}
```
