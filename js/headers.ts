// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util";
import { Headers, HeadersInit } from "./dom_types";

export type DenoHeadersInit = HeadersInit | DenoHeaders;

export class DenoHeaders implements Headers {
  private readonly headerList: [string, string][] = []; // [name, value]
  length: number;

  constructor(init?: DenoHeadersInit) {
    if (init) {
      this._fill(init);
    }
    this.length = this.headerList.length;
  }

  private _append(header: [string, string]): void {
    this._appendToHeaderList(header);
  }

  private _appendToHeaderList(header: [string, string]): void {
    const lowerCaseName = header[0].toLowerCase();
    for (let i = 0; i < this.headerList.length; ++i) {
      if (this.headerList[i][0].toLowerCase() === lowerCaseName) {
        header[0] = this.headerList[i][0];
      }
    }
    this.headerList.push(header);
  }

  private _fill(init: DenoHeadersInit): void {
    if (init instanceof DenoHeaders) {
      init.forEach((value, name) => {
        this._append([name, value]);
      });
    } else if (Array.isArray(init)) {
      for (let i = 0; i < init.length; ++i) {
        const header = init[i];
        if (header.length !== 2) {
          throw new TypeError("Failed to construct 'Headers': Invalid value");
        }
        this._append([header[0], header[1]]);
      }
    } else {
      for (const key in init) {
        this._append([key, init[key]]);
      }
    }
  }

  append(name: string, value: string): void {
    this._appendToHeaderList([name, value]);
  }

  delete(name: string): void {
    const idx = this.headerList.findIndex(function(h) {
      return h[0] == name.toLowerCase();
    });
    if (idx >= 0) this.headerList.splice(idx, 1);
  }

  get(name: string): string | null {
    for (const header of this.headerList) {
      if (header[0].toLowerCase() === name.toLowerCase()) {
        return header[1];
      }
    }
    return null;
  }

  has(name: string): boolean {
    assert(false, "Implement me");
    return false;
  }

  set(name: string, value: string): void {
    assert(false, "Implement me");
  }

  entries(): IterableIterator<[string, string]> {
    return new DenoHeadersIterator(this.headerList);
  }

  keys(): IterableIterator<string> {
    return new DenoKeysIterator(this.headerList);
  }

  values(): IterableIterator<string> {
    return new DenoValuesIterator(this.headerList);
  }

  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ): void {
    const it = this[Symbol.iterator]();
    let cur = it.next();
    while (!cur.done) {
      const [name, value] = cur.value;
      callbackfn(value, name, this);
      cur = it.next();
    }
  }

  [Symbol.iterator](): IterableIterator<[string, string]> {
    return new DenoHeadersIterator(this.headerList);
  }
}

class DenoHeadersIterator implements IterableIterator<[string, string]> {
  headers: [string, string][];
  private index: number;
  constructor(headers: [string, string][]) {
    this.headers = headers;
    this.index = 0;
  }

  next(): IteratorResult<[string, string]> {
    if (this.index >= this.headers.length) {
      return { value: undefined, done: true };
    }
    return { value: this.headers[this.index++], done: false };
  }

  [Symbol.iterator]() {
    return this;
  }
}

class DenoKeysIterator implements IterableIterator<string> {
  headers: [string, string][];
  private index: number;
  constructor(headers: [string, string][]) {
    this.headers = headers;
    this.index = 0;
  }

  next(): IteratorResult<string> {
    if (this.index >= this.headers.length) {
      return { value: undefined, done: true };
    }
    return { value: this.headers[this.index++][0], done: false };
  }

  [Symbol.iterator]() {
    return this;
  }
}

class DenoValuesIterator implements IterableIterator<string> {
  headers: [string, string][];
  private index: number;
  constructor(headers: [string, string][]) {
    this.headers = headers;
    this.index = 0;
  }

  next(): IteratorResult<string> {
    if (this.index >= this.headers.length) {
      return { value: undefined, done: true };
    }
    return { value: this.headers[this.index++][1], done: false };
  }

  [Symbol.iterator]() {
    return this;
  }
}
