// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
  const foo = "foo";
  const bar = 123;
  function blah(a) {
    Deno.core.print(a);
  }
  return { foo, bar, blah };
})();
