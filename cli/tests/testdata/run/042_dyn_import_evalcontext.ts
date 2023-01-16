// @ts-expect-error "Deno.core" is not a public interface
Deno.core.evalContext(
  "(async () => console.log(await import('./subdir/mod4.js')))()",
);
