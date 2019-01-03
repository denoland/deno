// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import * as blob from "./blob";
import * as domFile from "./dom_file";
import { DomIterableMixin } from "./mixins/dom_iterable";
import { requiredArguments } from "./util";

const dataSymbol = Symbol("data");

class FormDataBase {
  private [dataSymbol]: Array<[string, domTypes.FormDataEntryValue]> = [];

  /** Appends a new value onto an existing key inside a `FormData`
   * object, or adds the key if it does not already exist.
   *
   *       formData.append('name', 'first');
   *       formData.append('name', 'second');
   */
  append(name: string, value: string): void;
  append(name: string, value: blob.DenoBlob, filename?: string): void;
  append(name: string, value: string | blob.DenoBlob, filename?: string): void {
    requiredArguments("FormData.append", arguments.length, 2);
    name = String(name);
    if (value instanceof blob.DenoBlob) {
      const dfile = new domFile.DenoFile([value], filename || name);
      this[dataSymbol].push([name, dfile]);
    } else {
      this[dataSymbol].push([name, String(value)]);
    }
  }

  /** Deletes a key/value pair from a `FormData` object.
   *
   *       formData.delete('name');
   */
  delete(name: string): void {
    requiredArguments("FormData.delete", arguments.length, 1);
    name = String(name);
    let i = 0;
    while (i < this[dataSymbol].length) {
      if (this[dataSymbol][i][0] === name) {
        this[dataSymbol].splice(i, 1);
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
    requiredArguments("FormData.getAll", arguments.length, 1);
    name = String(name);
    const values = [];
    for (const entry of this[dataSymbol]) {
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
    requiredArguments("FormData.get", arguments.length, 1);
    name = String(name);
    for (const entry of this[dataSymbol]) {
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
    requiredArguments("FormData.has", arguments.length, 1);
    name = String(name);
    return this[dataSymbol].some(entry => entry[0] === name);
  }

  /** Sets a new value for an existing key inside a `FormData` object, or
   * adds the key/value if it does not already exist.
   * ref: https://xhr.spec.whatwg.org/#dom-formdata-set
   *
   *       formData.set('name', 'value');
   */
  set(name: string, value: string): void;
  set(name: string, value: blob.DenoBlob, filename?: string): void;
  set(name: string, value: string | blob.DenoBlob, filename?: string): void {
    requiredArguments("FormData.set", arguments.length, 2);
    name = String(name);

    // If there are any entries in the context object’s entry list whose name
    // is name, replace the first such entry with entry and remove the others
    let found = false;
    let i = 0;
    while (i < this[dataSymbol].length) {
      if (this[dataSymbol][i][0] === name) {
        if (!found) {
          if (value instanceof blob.DenoBlob) {
            const dfile = new domFile.DenoFile([value], filename || name);
            this[dataSymbol][i][1] = dfile;
          } else {
            this[dataSymbol][i][1] = String(value);
          }
          found = true;
        } else {
          this[dataSymbol].splice(i, 1);
          continue;
        }
      }
      i++;
    }

    // Otherwise, append entry to the context object’s entry list.
    if (!found) {
      if (value instanceof blob.DenoBlob) {
        const dfile = new domFile.DenoFile([value], filename || name);
        this[dataSymbol].push([name, dfile]);
      } else {
        this[dataSymbol].push([name, String(value)]);
      }
    }
  }
}

// tslint:disable-next-line:variable-name
export class FormData extends DomIterableMixin<
  string,
  domTypes.FormDataEntryValue,
  typeof FormDataBase
>(FormDataBase, dataSymbol) {}
