const [, errorInfo] = Deno.core.evalContext('throw new DOMException("foo")');
console.log(errorInfo);
