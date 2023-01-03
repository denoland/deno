// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const client = Deno.createHttpClient({
  proxy: {
    url: "http://localhost:4555",
    basicAuth: { username: "username", password: "password" },
  },
});

const res = await fetch(
  "http://localhost:4545/run/045_mod.ts",
  { client },
);
console.log(`Response http: ${await res.text()}`);

client.close();
