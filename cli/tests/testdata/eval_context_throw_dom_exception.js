const [, errorInfo] = Deno.core.evalContext(
  'throw new DOMException("foo")',
  "file:///test.js",
);
console.log(errorInfo);
