// @ts-expect-error "Deno[Deno.internal].core" is not a public interface
Deno[Deno.internal].core.evalContext(
  "(async () => console.log(await import('./subdir/mod4.js')))()",
  new URL("..", import.meta.url).href,
);
