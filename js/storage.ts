// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import { readFileSync, _compiler, File, open } from "./deno";
import { TextDecoder, btoa } from "./text_encoding";

class Storage implements domTypes.Storage {
  protected data: Map<string, string> = new Map();

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

class LocalStorage extends Storage {
  private file: Promise<File>;

  constructor(fileName: string) {
    super();

    const localStorageFile = btoa(fileName);
    this.unserialize(readFileSync(localStorageFile));
    this.file = open(localStorageFile, "w");
  }

  private unserialize(data: Uint8Array) {
    const decoder = new TextDecoder();
    const decodedData = JSON.parse(decoder.decode(data)) as {
      [key: string]: string;
    };
    Object.entries(decodedData).forEach(([key, value]) => {
      super.setItem(key, value);
    });
  }

  private serialize(): Uint8Array {
    const data: { [key: string]: string } = {};
    for (const [key, value] of this.data) {
      data[key] = value;
    }
    const encoder = new TextEncoder();
    return encoder.encode(JSON.stringify(data));
  }

  clear() {
    super.clear();
    this.file.then(file => file.write(Uint8Array.from([])));
  }
  removeItem(keyName: string) {
    this.removeItem(keyName);
    this.file.then(file => file.write(this.serialize()));
  }
  setItem(keyName: string, keyValue: string) {
    super.setItem(keyName, keyValue);
    this.file.then(file => file.write(this.serialize()));
  }
}

let localStorage: LocalStorage | null = null;

export const getLocaleStorage: () => Storage = () =>
  localStorage ||
  (localStorage = new LocalStorage(
    _compiler.DenoCompiler.instance().getScriptFileNames()[0]
  ));
export const sessionStorage = new Storage();
