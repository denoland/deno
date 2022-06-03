const [, errorInfo] = Deno.core.opSync(
  "op_eval_context",
  'throw new DOMException("foo")',
);
console.log(errorInfo);
