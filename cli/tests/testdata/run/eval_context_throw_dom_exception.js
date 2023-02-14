const [, errorInfo] = Deno[Deno.internal].core.evalContext(
  'throw new DOMException("foo")',
);
console.log(errorInfo);
