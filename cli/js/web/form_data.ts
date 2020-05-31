// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as blob from "./blob.ts";
import * as domFile from "./dom_file.ts";
import { DomIterableMixin } from "./dom_iterable.ts";
import { requiredArguments } from "./util.ts";

const dataSymbol = Symbol("data");

class FormDataBase {
  [dataSymbol]: Array<[string, FormDataEntryValue]> = [];

  append(name: string, value: string): void;
  append(name: string, value: domFile.DomFileImpl): void;
  append(name: string, value: blob.DenoBlob, filename?: string): void;
  append(
    name: string,
    value: string | blob.DenoBlob | domFile.DomFileImpl,
    filename?: string
  ): void {
    requiredArguments("FormData.append", arguments.length, 2);
    name = String(name);
    if (value instanceof domFile.DomFileImpl) {
      this[dataSymbol].push([name, value]);
    } else if (value instanceof blob.DenoBlob) {
      const dfile = new domFile.DomFileImpl([value], filename || "blob", {
        type: value.type,
      });
      this[dataSymbol].push([name, dfile]);
    } else {
      this[dataSymbol].push([name, String(value)]);
    }
  }

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

  getAll(name: string): FormDataEntryValue[] {
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

  get(name: string): FormDataEntryValue | null {
    requiredArguments("FormData.get", arguments.length, 1);
    name = String(name);
    for (const entry of this[dataSymbol]) {
      if (entry[0] === name) {
        return entry[1];
      }
    }

    return null;
  }

  has(name: string): boolean {
    requiredArguments("FormData.has", arguments.length, 1);
    name = String(name);
    return this[dataSymbol].some((entry): boolean => entry[0] === name);
  }

  set(name: string, value: string): void;
  set(name: string, value: domFile.DomFileImpl): void;
  set(name: string, value: blob.DenoBlob, filename?: string): void;
  set(
    name: string,
    value: string | blob.DenoBlob | domFile.DomFileImpl,
    filename?: string
  ): void {
    requiredArguments("FormData.set", arguments.length, 2);
    name = String(name);

    // If there are any entries in the context object’s entry list whose name
    // is name, replace the first such entry with entry and remove the others
    let found = false;
    let i = 0;
    while (i < this[dataSymbol].length) {
      if (this[dataSymbol][i][0] === name) {
        if (!found) {
          if (value instanceof domFile.DomFileImpl) {
            this[dataSymbol][i][1] = value;
          } else if (value instanceof blob.DenoBlob) {
            const dfile = new domFile.DomFileImpl([value], filename || "blob", {
              type: value.type,
            });
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
      if (value instanceof domFile.DomFileImpl) {
        this[dataSymbol].push([name, value]);
      } else if (value instanceof blob.DenoBlob) {
        const dfile = new domFile.DomFileImpl([value], filename || "blob", {
          type: value.type,
        });
        this[dataSymbol].push([name, dfile]);
      } else {
        this[dataSymbol].push([name, String(value)]);
      }
    }
  }

  get [Symbol.toStringTag](): string {
    return "FormData";
  }
}

export class FormDataImpl extends DomIterableMixin<
  string,
  FormDataEntryValue,
  typeof FormDataBase
>(FormDataBase, dataSymbol) {}

Object.defineProperty(FormDataImpl, "name", {
  value: "FormData",
  configurable: true,
});
