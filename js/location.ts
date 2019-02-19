// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { URL } from "./url";
import { notImplemented } from "./util";
import { Location } from "./dom_types";
import { window } from "./window";

export function setLocation(url: string): void {
  window.location = new LocationImpl(url);
  Object.freeze(window.location);
}

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
  assign(url: string): void {
    throw notImplemented();
  }
  reload(): void {
    throw notImplemented();
  }
  replace(url: string): void {
    throw notImplemented();
  }
}
