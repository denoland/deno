// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { customInspect } from "./deno.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";

interface StorageImpl {
  readonly length: number;
  key(index: number): string | null;
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
  clear(): void;
  // TODO: do not support yet
  // [key: string]: any;
  // [index: number]: string;
}

const kvMapSymbol = Symbol("kv_map_symbol");
const localStorageOriginSymbol = Symbol("localstorage_origin_symbol");

interface KVMap {
  [k: string]: string;
}

export class Storage implements StorageImpl {
  get length(): number {
    return 0;
  }
  key(_index: number): string | null {
    return null;
  }
  getItem(_key: string): string | null {
    return null;
  }
  setItem(_key: string, _value: string): void {
    return;
  }
  removeItem(_key: string): void {
    return;
  }
  clear(): void {
    return;
  }
  [customInspect](): string {
    return `Storage {}`;
  }
}

export class SessionStorage extends Storage {
  private [kvMapSymbol]: KVMap = {};
  get length(): number {
    return Object.keys(this[kvMapSymbol]).length;
  }
  key(index: number): string | null {
    const map = this[kvMapSymbol];

    const keys = Object.keys(map);
    const key = keys[index];

    if (key === undefined) {
      return null;
    }

    return map[key];
  }
  getItem(key: string): string | null {
    const val = this[kvMapSymbol][key];
    return val === undefined ? null : val;
  }

  setItem(key: string, value: string): void {
    this[kvMapSymbol][key] = value.toString();
    return;
  }

  removeItem(key: string): void {
    delete this[kvMapSymbol][key];
    return;
  }

  clear(): void {
    for (const key in this[kvMapSymbol]) {
      delete this[kvMapSymbol][key];
    }
    return;
  }

  [customInspect](): string {
    const keys = Object.keys(this[kvMapSymbol]);
    const str = keys
      .map((key: string) => {
        return `${key}: "${this.getItem(key)}"`;
      })
      .concat([`length: ${this.length}`])
      .join(", ");
    return `Storage {${str}}`;
  }
}

export class LocalStorage extends Storage {
  // TODO: dispatch data with origin.
  private [localStorageOriginSymbol] = "";
  constructor(origin = "") {
    super();
    this[localStorageOriginSymbol] = origin;
  }
  get length(): number {
    return sendSync(dispatch.OP_LOCALSTORAGE_GET_LEN);
  }
  key(index: number): string | null {
    const res = sendSync(dispatch.OP_LOCALSTORAGE_KEY, {
      index: index
    });

    return res.value;
  }
  getItem(key: string): string | null {
    const res = sendSync(dispatch.OP_LOCALSTORAGE_GET_ITEM, { key: key });

    return res.value;
  }

  setItem(key: string, value: string): void {
    sendSync(dispatch.OP_LOCALSTORAGE_SET_ITEM, {
      key: key,
      value: value
    });
  }

  removeItem(key: string): void {
    sendSync(dispatch.OP_LOCALSTORAGE_REMOVE_ITEM, { key: key });
  }

  clear(): void {
    sendSync(dispatch.OP_LOCALSTORAGE_CLEAN);
  }

  [customInspect](): string {
    // TODO: finish this.
    return `Storage {}`;
  }
}
