// @ts-expect-error "Deno.core" is not a public interface
Deno.core.opSync(
  "op_eval_context",
  "(async () => console.log(await import('./subdir/mod4.js')))()",
);
