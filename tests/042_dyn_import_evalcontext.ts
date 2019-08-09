// @ts-ignore
Deno.core.evalContext(
  "(async () => console.log(await import('./tests/subdir/mod4.js')))()"
);
