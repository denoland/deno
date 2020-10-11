// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import init, {
    source,
    parse_date,
    parse_addr,
    DenoAddr,
} from "./wasm.js";

await init(source);

const encoder = new TextEncoder();

export function parseDate(date: string) {
    let view = encoder.encode(date);
    return parse_date(view)
}

console.log(parse_addr(encoder.encode("John Doe <john@doe.com>"))[0])
