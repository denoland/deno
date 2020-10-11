// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import init, {
    source,
    parse_date,
    parse_addr
} from "./wasm.js";

await init(source);

const encoder = new TextEncoder();

export function parseDate(date: string) {
    let view = encoder.encode(date);
    return parse_date(view)
}

export function parseAddr(addr: string) {
    let view = encoder.encode(addr);
    let res = parse_addr(view);
    return res
}

