// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
import { op_get_extras_binding_object } from "ext:core/ops";

const {
  SafeWeakMap,
} = primordials;

const {
  getContinuationPreservedEmbedderData,
  setContinuationPreservedEmbedderData,
} = op_get_extras_binding_object();

let counter = 0;

export const getAsyncContext = getContinuationPreservedEmbedderData;
export const setAsyncContext = setContinuationPreservedEmbedderData;

export class AsyncVariable {
  #id = counter++;
  #data = new SafeWeakMap();

  enter(value) {
    const previousContextMapping = getAsyncContext();
    const entry = { id: this.#id };
    const asyncContextMapping = {
      __proto__: null,
      ...previousContextMapping,
      [this.#id]: entry,
    };
    this.#data.set(entry, value);
    setAsyncContext(asyncContextMapping);
    return previousContextMapping;
  }

  get() {
    const current = getAsyncContext();
    const entry = current?.[this.#id];
    if (entry) {
      return this.#data.get(entry);
    }
    return undefined;
  }
}
