// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";

class Storage implements domTypes.Storage {
  private data: Map<string, string> = new Map();

  get length() {
    return this.data.size;
  }
  clear() {
    this.data.clear();
  }
  getItem(keyName: string) {
    return this.data.get(keyName) || null;
  }
  key(index: number) {
    let ctr = 0;
    for (const key of this.data.keys()) {
      if (ctr++ === index) {
        return key;
      }
    }
    return null;
  }
  removeItem(keyName: string) {
    this.data.delete(keyName);
  }
  setItem(keyName: string, keyValue: string) {
    this.data.set(keyName, keyValue);
  }
}

export const sessionStorage = new Storage();
