// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import init, { parse_addr, parse_date, source } from "./wasm.js";

await init(source);

const encoder = new TextEncoder();

export function parseDate(date: string) {
  const view = encoder.encode(date);
  return parse_date(view);
}

export function parseAddr(addr: string) {
  const view = encoder.encode(addr);
  return parse_addr(view);
}
