// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import init, {
    source,
    parse_date,
    parse_addr_single,
} from "./wasm.js";

await init(source);

const encoder = new TextEncoder();

export function parseDate(date: string) {
    let view = encoder.encode(date);
    return parse_date(view)
}

console.log(parse_addr_single(encoder.encode("John Doe <john@doe.com>, John Doe <john@doe.com>")))
