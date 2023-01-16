// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const { DOMException } = window.__bootstrap.domException;
  const {
    ArrayBuffer,
    ArrayBufferPrototype,
    ArrayBufferIsView,
    DataViewPrototype,
    ObjectPrototypeIsPrototypeOf,
    TypedArrayPrototypeSlice,
    TypeErrorPrototype,
    WeakMap,
    WeakMapPrototypeSet,
  } = window.__bootstrap.primordials;

  const objectCloneMemo = new WeakMap();

  function cloneArrayBuffer(
    srcBuffer,
    srcByteOffset,
    srcLength,
    _cloneConstructor,
  ) {
    // this function fudges the return type but SharedArrayBuffer is disabled for a while anyway
    return TypedArrayPrototypeSlice(
      srcBuffer,
      srcByteOffset,
      srcByteOffset + srcLength,
    );
  }

  /** Clone a value in a similar way to structured cloning.  It is similar to a
   * StructureDeserialize(StructuredSerialize(...)). */
  function structuredClone(value) {
    // Performance optimization for buffers, otherwise
    // `serialize/deserialize` will allocate new buffer.
    if (ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, value)) {
      const cloned = cloneArrayBuffer(
        value,
        0,
        value.byteLength,
        ArrayBuffer,
      );
      WeakMapPrototypeSet(objectCloneMemo, value, cloned);
      return cloned;
    }
    if (ArrayBufferIsView(value)) {
      const clonedBuffer = structuredClone(value.buffer);
      // Use DataViewConstructor type purely for type-checking, can be a
      // DataView or TypedArray.  They use the same constructor signature,
      // only DataView has a length in bytes and TypedArrays use a length in
      // terms of elements, so we adjust for that.
      let length;
      if (ObjectPrototypeIsPrototypeOf(DataViewPrototype, view)) {
        length = value.byteLength;
      } else {
        length = value.length;
      }
      return new (value.constructor)(
        clonedBuffer,
        value.byteOffset,
        length,
      );
    }

    try {
      return core.deserialize(core.serialize(value));
    } catch (e) {
      if (ObjectPrototypeIsPrototypeOf(TypeErrorPrototype, e)) {
        throw new DOMException(e.message, "DataCloneError");
      }
      throw e;
    }
  }

  window.__bootstrap.structuredClone = structuredClone;
})(globalThis);
