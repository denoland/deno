// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
const url_ = Deno.args[0];
const res = await fetch(url_);

// TODO(ry) Re-enable streaming in this example.
// Originally we did: await Deno.copy(res.body, Deno.stdout);
// But maybe more JS-y would be: res.pipeTo(Deno.stdout);

const body = new Uint8Array(await res.arrayBuffer());
await Deno.stdout.write(body);
