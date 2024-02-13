#!/usr/bin/env -S deno run --allow-net --allow-read --allow-write
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// JSON version of the full canonical list of named HTML entities
// https://html.spec.whatwg.org/multipage/named-characters.html
import entityList from "https://html.spec.whatwg.org/entities.json" assert {
  type: "json",
};

const data = Object.fromEntries(
  Object.entries(entityList).map(([k, v]) => [k, v.characters]),
);

await Deno.writeTextFile(
  new URL(import.meta.resolve("../named_entity_list.json")),
  JSON.stringify(data, null, 2) + "\n",
);
