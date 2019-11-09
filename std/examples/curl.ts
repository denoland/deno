// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const url_ = Deno.args[1];
const res = await fetch(url_);
await Deno.copy(Deno.stdout, res.body);
