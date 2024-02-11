const [, errorInfo] = Deno[Deno.internal].core.evalContext(
  'throw new DOMException("foo")',
  new URL("..", import.meta.url).href,
);
console.log(errorInfo);
