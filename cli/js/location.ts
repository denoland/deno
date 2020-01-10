// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { URL } from "./url.ts";
import { notImplemented } from "./util.ts";
import { Location } from "./dom_types.ts";
import { window } from "./window.ts";

export class LocationImpl implements Location {
  constructor(url: string) {
    const u = new URL(url);
    this.url = u;
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

  private url: URL;

  toString(): string {
    return this.url.toString();
  }

  readonly ancestorOrigins: string[] = [];
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

export function setLocation(url: string): void {
  window.location = new LocationImpl(url);
  Object.freeze(window.location);
}
