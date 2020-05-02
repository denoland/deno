// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import {
  localStorageInit,
  localStorageClear,
  localStorageGetItem,
  localStorageGetLength,
  localStorageRemoveItem,
  localStorageSetItem,
} from "../ops/local_storage.ts";
import { DOMExceptionImpl } from "./dom_exception.ts";

const data = Symbol("internal Storage data");

class NonPersistantStorageImpl implements Storage {
  private [data]: Map<string, string> = new Map();

  get length(): number {
    return this[data].size;
  }
  key(index: number): string | null {
    if (index >= this[data].size) {
      return null;
    } else {
      return Array.from(this[data].keys())[index];
    }
  }
  getItem(keyName: string): string | null {
    return this[data].get(keyName) || null;
  }
  setItem(keyName: string, keyValue: string): void {
    this[data].set(keyName, keyValue);
  }
  removeItem(keyName: string): void {
    this[data].delete(keyName);
  }
  clear(): void {
    this[data].clear();
  }
}

class PersistantStorageImpl implements Storage {
  get length(): number {
    return localStorageGetLength();
  }
  key(index: number): string | null {
    throw "Unimplemented";
  }
  getItem(keyName: string): string | null {
    return localStorageGetItem(keyName);
  }
  setItem(keyName: string, keyValue: string): void {
    if ("error" in localStorageSetItem(keyName, keyValue)) {
      throw new DOMExceptionImpl("Failed to set item", "QuotaExceededError");
    }
  }
  removeItem(keyName: string): void {
    localStorageRemoveItem(keyName);
  }
  clear(): void {
    // TODO: make it atomic somehow
    localStorageClear();
  }
}

const storageHandler = {
  deleteProperty(target: Storage, key: string): boolean {
    target.removeItem(key);
    return true;
  },
  has(target: Storage, key: string): boolean {
    return target.getItem(key) !== null || key in target;
  },
  get(target: Storage, key: string): string | null | number | void {
    if ("undefined" !== typeof target[key]) {
      // @ts-ignore
      return Reflect.get(...arguments); // eslint-disable-line prefer-rest-params
    } else {
      return target.getItem(key);
    }
  },
  set(target: Storage, key: string, value: string): boolean {
    try {
      target.setItem(key, value);
      return true;
    } catch {
      return false;
    }
  },
};

export const sessionStorage = new Proxy(
  new NonPersistantStorageImpl(),
  storageHandler
);
export const localStorage = new Proxy(
  new PersistantStorageImpl(),
  storageHandler
);
export { localStorageInit };
