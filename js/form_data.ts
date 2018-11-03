/**
 * @module deno
 * @private
 */
import { stringify } from "query-string";
import { FormData, FormDataEntryValue } from "./dom_types";
import { notImplemented } from "./util";

/**
 * Class representing a fetch response.
 * @hidden
 */
export default class FlyFormData implements FormData {
  private _data: Map<string, string[]>;

  constructor() {
    this._data = new Map<string, string[]>();
  }

  append(name: string, value: string) {
    let vals: string[];
    const currentVals = this._data.get(name);
    if (currentVals == undefined) {
      vals = [value];
    } else {
      vals = currentVals.concat([value]);
    }
    this._data.set(name, vals);
  }

  delete(name: string) {
    this._data.delete(name);
  }

  entries(): IterableIterator<[string, string[]]> {
    return this._data.entries();
  }

  forEach(
    callbackfn: (
      value: FormDataEntryValue,
      key: string,
      parent: FormData
    ) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ) {
    notImplemented();
  }

  get(name: string): string | null {
    const vals = this._data.get(name);
    if (vals == undefined) {
      return null;
    }
    return vals[0];
  }

  getAll(name: string): string[] {
    const vals = this._data.get(name);
    if (vals == undefined) {
      return [];
    }
    return vals;
  }

  has(name: string): boolean {
    return this._data.has(name);
  }

  keys(): IterableIterator<string> {
    return this._data.keys();
  }

  set(name: string, value: string) {
    this._data.set(name, [value]);
  }

  values(): IterableIterator<string> {
    // this._data.values() doesn't flatten arrays of arrays
    let that = this;
    return (function*() {
      for (let vals of that._data.values()) {
        if (Array.isArray(vals)) {
          for (let val of vals) {
            yield val;
          }
        } else {
          yield vals;
        }
      }
    })();
  }

  toString(): string {
    const output: string[] = [];
    this._data.forEach((value, key) => {
      output.push(stringify({ [`${key}`]: value }));
    });
    return output.join("&");
  }
}
