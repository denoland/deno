// Copyright 2018-2025 the Deno authors. MIT license.
import bin from "./test.bin" with { type: "bytes" };
import txt from "./test.txt" with { type: "text" };
import json from "./test.json" with { type: "json" };

console.log(bin);
console.log(JSON.stringify(txt));
console.log(JSON.stringify(json));
