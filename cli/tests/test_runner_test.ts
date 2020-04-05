// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
Deno.test(function fail1() {
  throw new Error("fail1 assertion");
});

Deno.test(function fail2() {
  throw new Error("fail2 assertion");
});

Deno.test(function success1() {});

Deno.test(function fail3() {
  throw new Error("fail3 assertion");
});
