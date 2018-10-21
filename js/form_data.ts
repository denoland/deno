// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import { DenoBlob } from "./blob";
import { DenoFile } from "./file";

export class FormData implements domTypes.FormData {
  private data: Array<[string, domTypes.FormDataEntryValue]> = [];

  /** Appends a new value onto an existing key inside a `FormData`
   * object, or adds the key if it does not already exist.
   *
   *       formData.append('name', 'first');
   *       formData.append('name', 'second');
   */
  append(name: string, value: string): void;
  append(name: string, value: DenoBlob, filename?: string): void;
  append(name: string, value: string | DenoBlob, filename?: string): void {
    if (value instanceof DenoBlob) {
      const file = new DenoFile([value], filename || name);
      this.data.push([name, file]);
    } else {
      this.data.push([name, value]);
    }
  }

  /** Deletes a key/value pair from a `FormData` object.
   *
   *       formData.delete('name');
   */
  delete(name: string): void {
    let i = 0;
    while (i < this.data.length) {
      if (this.data[i][0] === name) {
        this.data.splice(i, 1);
      } else {
        i++;
      }
    }
  }

  /** Returns an array of all the values associated with a given key
   * from within a `FormData`.
   *
   *       formData.getAll('name');
   */
  getAll(name: string): domTypes.FormDataEntryValue[] {
    const values = [];
    for (const entry of this.data) {
      if (entry[0] === name) {
        values.push(entry[1]);
      }
    }

    return values;
  }

  /** Returns the first value associated with a given key from within a
   * `FormData` object.
   *
   *       formData.get('name');
   */
  get(name: string): domTypes.FormDataEntryValue | null {
    for (const entry of this.data) {
      if (entry[0] === name) {
        return entry[1];
      }
    }

    return null;
  }

  /** Returns a boolean stating whether a `FormData` object contains a
   * certain key/value pair.
   *
   *       formData.has('name');
   */
  has(name: string): boolean {
    return this.data.some(entry => entry[0] === name);
  }

  /** Sets a new value for an existing key inside a `FormData` object, or
   * adds the key/value if it does not already exist.
   *
   *       formData.set('name', 'value');
   */
  set(name: string, value: string): void;
  set(name: string, value: DenoBlob, filename?: string): void;
  set(name: string, value: string | DenoBlob, filename?: string): void {
    this.delete(name);
    if (value instanceof DenoBlob) {
      const file = new DenoFile([value], filename || name);
      this.data.push([name, file]);
    } else {
      this.data.push([name, value]);
    }
  }

  /** Calls a function for each element contained in this object in
   * place and return undefined. Optionally accepts an object to use
   * as this when executing callback as second argument.
   *
   *       formData.forEach((value, key, parent) => {
   *         console.log(value, key, parent);
   *       });
   */
  forEach(
    callbackfn: (
      value: domTypes.FormDataEntryValue,
      key: string,
      parent: FormData
    ) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ) {
    if (typeof thisArg !== "undefined") {
      callbackfn = callbackfn.bind(thisArg);
    }
    for (const [key, value] of this.entries()) {
      callbackfn(value, key, this);
    }
  }

  /** Returns an iterator allowing to go through all keys contained
   * in this object.
   *
   *       for (const key of formData.keys()) {
   *         console.log(key);
   *       }
   */
  *keys(): Iterable<string> {
    for (const entry of this.data) {
      yield entry[0];
    }
  }

  /** Returns an iterator allowing to go through all values contained
   * in this object.
   *
   *       for (const value of formData.values()) {
   *         console.log(value);
   *       }
   */
  *values(): Iterable<domTypes.FormDataEntryValue> {
    for (const entry of this.data) {
      yield entry[1];
    }
  }

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       for (const [key, value] of formData.entries()) {
   *         console.log(key, value);
   *       }
   */
  *entries(): Iterable<[string, domTypes.FormDataEntryValue]> {
    yield* this.data;
  }

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       for (const [key, value] of formData[Symbol.iterator]()) {
   *         console.log(key, value);
   *       }
   */
  *[Symbol.iterator](): Iterable<[string, domTypes.FormDataEntryValue]> {
    yield* this.data;
  }
}
