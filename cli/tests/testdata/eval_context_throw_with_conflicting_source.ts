// deno-lint-ignore no-explicit-any
const [, errorInfo] = (Deno as any).core.evalContext(
  '/* aaaaaaaaaaaaaaaaa */ throw new Error("foo")',
  new URL("eval_context_conflicting_source.ts", import.meta.url).href,
);
throw errorInfo.thrown;
