// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { URL } from "./url.ts";
import { notImplemented } from "../util.ts";
import { DOMStringList, Location } from "./dom_types.ts";
import { getDOMStringList } from "./dom_util.ts";

export class LocationImpl implements Location {
  #url: URL;

  constructor(url: string) {
    const u = new URL(url);
    this.#url = u;
    this.hash = u.hash;
    this.host = u.host;
    this.href = u.href;
    this.hostname = u.hostname;
    this.origin = u.protocol + "//" + u.host;
    this.pathname = u.pathname;
    this.protocol = u.protocol;
    this.port = u.port;
    this.search = u.search;
  }

  toString(): string {
    return this.#url.toString();
  }

  readonly ancestorOrigins: DOMStringList = getDOMStringList([]);
  hash: string;
  host: string;
  hostname: string;
  href: string;
  readonly origin: string;
  pathname: string;
  port: string;
  protocol: string;
  search: string;
  assign(_url: string): void {
    throw notImplemented();
  }
  reload(): void {
    throw notImplemented();
  }
  replace(_url: string): void {
    throw notImplemented();
  }
}

/** Sets the `window.location` at runtime.
 * @internal */
export function setLocation(url: string): void {
  globalThis.location = new LocationImpl(url);
  Object.freeze(globalThis.location);
}
