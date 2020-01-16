// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const url_ = Deno.args[0];
const res = await fetch(url_);
await Deno.copy(Deno.stdout, res.body);
