async function work(): Promise<void> {}

// A floating promise: `work()` returns a Promise that is never awaited or
// handled. Only a type-aware rule (no-floating-promises) can catch this.
work();

// This one is fine.
await work();
