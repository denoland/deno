async function work(): Promise<void> {}

// deno-lint-ignore no-floating-promises
work();

await work();
