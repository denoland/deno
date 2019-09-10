// @ts-ignore
Deno.core.evalContext(
  "(async () => console.log(await import('./subdir/mod4.js')))()"
);
