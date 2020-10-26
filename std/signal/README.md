# signal

signal is a module used to capture and monitor OS signals.

# usage

The following functions are exposed in `mod.ts`:

## signal

Generates an AsyncIterable which can be awaited on for one or more signals.
`dispose()` can be called when you are finished waiting on the events.

```typescript
import { signal } from "https://deno.land/std/signal/mod.ts";
const sig = signal(Deno.Signal.SIGUSR1, Deno.Signal.SIGINT);
setTimeout(() => {}, 5000); // Prevents exiting immediately.

for await (const _ of sig) {
  // ..
}

// At some other point in your code when finished listening:
sig.dispose();
```

## onSignal

Registers a callback function to be called on triggering of a signal event.

```typescript
import { onSignal } from "https://deno.land/std/signal/mod.ts";

const handle = onSignal(Deno.Signal.SIGINT, () => {
  // ...
  handle.dispose(); // de-register from receiving further events.
});
```
