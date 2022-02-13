// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference no-default-lib="true" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference lib="esnext" />

"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;
  const { forgivingBase64Encode } = window.__bootstrap.infra;
  const { decode, TextDecoder } = window.__bootstrap.encoding;
  const { parseMimeType } = window.__bootstrap.mimesniff;
  const { DOMException } = window.__bootstrap.domException;
  const {
    ArrayPrototypeJoin,
    ArrayPrototypeMap,
    ArrayPrototypePush,
    ArrayPrototypeReduce,
    FunctionPrototypeCall,
    Map,
    MapPrototypeGet,
    MapPrototypeSet,
    ObjectDefineProperty,
    ObjectPrototypeIsPrototypeOf,
    queueMicrotask,
    SafeArrayIterator,
    StringFromCodePoint,
    Symbol,
    TypedArrayPrototypeSet,
    TypeError,
    Uint8Array,
    Uint8ArrayPrototype,
  } = window.__bootstrap.primordials;

  const state = Symbol("[[state]]");
  const result = Symbol("[[result]]");
  const error = Symbol("[[error]]");
  const aborted = Symbol("[[aborted]]");
  const handlerSymbol = Symbol("eventHandlers");

  class FileReader extends EventTarget {
    /** @type {"empty" | "loading" | "done"} */
    [state] = "empty";
    /** @type {null | string | ArrayBuffer} */
    [result] = null;
    /** @type {null | DOMException} */
    [error] = null;
    /** @type {null | {aborted: boolean}} */
    [aborted] = null;

    /**
     * @param {Blob} blob
     * @param {{kind: "ArrayBuffer" | "Text" | "DataUrl" | "BinaryString", encoding?: string}} readtype
     */
    #readOperation(blob, readtype) {
      // 1. If fr’s state is "loading", throw an InvalidStateError DOMException.
      if (this[state] === "loading") {
        throw new DOMException(
          "Invalid FileReader state.",
          "InvalidStateError",
        );
      }
      // 2. Set fr’s state to "loading".
      this[state] = "loading";
      // 3. Set fr’s result to null.
      this[result] = null;
      // 4. Set fr’s error to null.
      this[error] = null;

      // We set this[aborted] to a new object, and keep track of it in a
      // separate variable, so if a new read operation starts while there are
      // remaining tasks from a previous aborted operation, the new operation
      // will run while the tasks from the previous one are still aborted.
      const abortedState = this[aborted] = { aborted: false };

      // 5. Let stream be the result of calling get stream on blob.
      const stream /*: ReadableStream<ArrayBufferView>*/ = blob.stream();

      // 6. Let reader be the result of getting a reader from stream.
      const reader = stream.getReader();

      // 7. Let bytes be an empty byte sequence.
      /** @type {Uint8Array[]} */
      const chunks = [];

      // 8. Let chunkPromise be the result of reading a chunk from stream with reader.
      let chunkPromise = reader.read();

      // 9. Let isFirstChunk be true.
      let isFirstChunk = true;

      // 10 in parallel while true
      (async () => {
        while (!abortedState.aborted) {
          // 1. Wait for chunkPromise to be fulfilled or rejected.
          try {
            const chunk = await chunkPromise;
            if (abortedState.aborted) return;

            // 2. If chunkPromise is fulfilled, and isFirstChunk is true, queue a task to fire a progress event called loadstart at fr.
            if (isFirstChunk) {
              // TODO(lucacasonato): this is wrong, should be HTML "queue a task"
              queueMicrotask(() => {
                if (abortedState.aborted) return;
                // fire a progress event for loadstart
                const ev = new ProgressEvent("loadstart", {});
                this.dispatchEvent(ev);
              });
            }
            // 3. Set isFirstChunk to false.
            isFirstChunk = false;

            // 4. If chunkPromise is fulfilled with an object whose done property is false
            // and whose value property is a Uint8Array object, run these steps:
            if (
              !chunk.done &&
              ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, chunk.value)
            ) {
              ArrayPrototypePush(chunks, chunk.value);

              // TODO(bartlomieju): (only) If roughly 50ms have passed since last progress
              {
                const size = ArrayPrototypeReduce(
                  chunks,
                  (p, i) => p + i.byteLength,
                  0,
                );
                const ev = new ProgressEvent("progress", {
                  loaded: size,
                });
                // TODO(lucacasonato): this is wrong, should be HTML "queue a task"
                queueMicrotask(() => {
                  if (abortedState.aborted) return;
                  this.dispatchEvent(ev);
                });
              }

              chunkPromise = reader.read();
            } // 5 Otherwise, if chunkPromise is fulfilled with an object whose done property is true, queue a task to run the following steps and abort this algorithm:
            else if (chunk.done === true) {
              // TODO(lucacasonato): this is wrong, should be HTML "queue a task"
              queueMicrotask(() => {
                if (abortedState.aborted) return;
                // 1. Set fr’s state to "done".
                this[state] = "done";
                // 2. Let result be the result of package data given bytes, type, blob’s type, and encodingName.
                const size = ArrayPrototypeReduce(
                  chunks,
                  (p, i) => p + i.byteLength,
                  0,
                );
                const bytes = new Uint8Array(size);
                let offs = 0;
                for (const chunk of chunks) {
                  TypedArrayPrototypeSet(bytes, chunk, offs);
                  offs += chunk.byteLength;
                }
                switch (readtype.kind) {
                  case "ArrayBuffer": {
                    this[result] = bytes.buffer;
                    break;
                  }
                  case "BinaryString":
                    this[result] = ArrayPrototypeJoin(
                      ArrayPrototypeMap(
                        [...new Uint8Array(bytes.buffer)],
                        (v) => StringFromCodePoint(v),
                      ),
                      "",
                    );
                    break;
                  case "Text": {
                    let decoder = undefined;
                    if (readtype.encoding) {
                      try {
                        decoder = new TextDecoder(readtype.encoding);
                      } catch {
                        // don't care about the error
                      }
                    }
                    if (decoder === undefined) {
                      const mimeType = parseMimeType(blob.type);
                      if (mimeType) {
                        const charset = MapPrototypeGet(
                          mimeType.parameters,
                          "charset",
                        );
                        if (charset) {
                          try {
                            decoder = new TextDecoder(charset);
                          } catch {
                            // don't care about the error
                          }
                        }
                      }
                    }
                    if (decoder === undefined) {
                      decoder = new TextDecoder();
                    }
                    this[result] = decode(bytes, decoder.encoding);
                    break;
                  }
                  case "DataUrl": {
                    const mediaType = blob.type || "application/octet-stream";
                    this[result] = `data:${mediaType};base64,${
                      forgivingBase64Encode(bytes)
                    }`;
                    break;
                  }
                }
                // 4.2 Fire a progress event called load at the fr.
                {
                  const ev = new ProgressEvent("load", {
                    lengthComputable: true,
                    loaded: size,
                    total: size,
                  });
                  this.dispatchEvent(ev);
                }

                // 5. If fr’s state is not "loading", fire a progress event called loadend at the fr.
                //Note: Event handler for the load or error events could have started another load, if that happens the loadend event for this load is not fired.
                if (this[state] !== "loading") {
                  const ev = new ProgressEvent("loadend", {
                    lengthComputable: true,
                    loaded: size,
                    total: size,
                  });
                  this.dispatchEvent(ev);
                }
              });
              break;
            }
          } catch (err) {
            // TODO(lucacasonato): this is wrong, should be HTML "queue a task"
            queueMicrotask(() => {
              if (abortedState.aborted) return;

              // chunkPromise rejected
              this[state] = "done";
              this[error] = err;

              {
                const ev = new ProgressEvent("error", {});
                this.dispatchEvent(ev);
              }

              //If fr’s state is not "loading", fire a progress event called loadend at fr.
              //Note: Event handler for the error event could have started another load, if that happens the loadend event for this load is not fired.
              if (this[state] !== "loading") {
                const ev = new ProgressEvent("loadend", {});
                this.dispatchEvent(ev);
              }
            });
            break;
          }
        }
      })();
    }

    #getEventHandlerFor(name) {
      webidl.assertBranded(this, FileReaderPrototype);

      const maybeMap = this[handlerSymbol];
      if (!maybeMap) return null;

      return MapPrototypeGet(maybeMap, name)?.handler ?? null;
    }

    #setEventHandlerFor(name, value) {
      webidl.assertBranded(this, FileReaderPrototype);

      if (!this[handlerSymbol]) {
        this[handlerSymbol] = new Map();
      }
      let handlerWrapper = MapPrototypeGet(this[handlerSymbol], name);
      if (handlerWrapper) {
        handlerWrapper.handler = value;
      } else {
        handlerWrapper = makeWrappedHandler(value);
        this.addEventListener(name, handlerWrapper);
      }

      MapPrototypeSet(this[handlerSymbol], name, handlerWrapper);
    }

    constructor() {
      super();
      this[webidl.brand] = webidl.brand;
    }

    /** @returns {number} */
    get readyState() {
      webidl.assertBranded(this, FileReaderPrototype);
      switch (this[state]) {
        case "empty":
          return FileReader.EMPTY;
        case "loading":
          return FileReader.LOADING;
        case "done":
          return FileReader.DONE;
        default:
          throw new TypeError("Invalid state");
      }
    }

    get result() {
      webidl.assertBranded(this, FileReaderPrototype);
      return this[result];
    }

    get error() {
      webidl.assertBranded(this, FileReaderPrototype);
      return this[error];
    }

    abort() {
      webidl.assertBranded(this, FileReaderPrototype);
      // If context object's state is "empty" or if context object's state is "done" set context object's result to null and terminate this algorithm.
      if (
        this[state] === "empty" ||
        this[state] === "done"
      ) {
        this[result] = null;
        return;
      }
      // If context object's state is "loading" set context object's state to "done" and set context object's result to null.
      if (this[state] === "loading") {
        this[state] = "done";
        this[result] = null;
      }
      // If there are any tasks from the context object on the file reading task source in an affiliated task queue, then remove those tasks from that task queue.
      // Terminate the algorithm for the read method being processed.
      if (this[aborted] !== null) {
        this[aborted].aborted = true;
      }

      // Fire a progress event called abort at the context object.
      const ev = new ProgressEvent("abort", {});
      this.dispatchEvent(ev);

      // If context object's state is not "loading", fire a progress event called loadend at the context object.
      if (this[state] !== "loading") {
        const ev = new ProgressEvent("loadend", {});
        this.dispatchEvent(ev);
      }
    }

    /** @param {Blob} blob */
    readAsArrayBuffer(blob) {
      webidl.assertBranded(this, FileReaderPrototype);
      const prefix = "Failed to execute 'readAsArrayBuffer' on 'FileReader'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      this.#readOperation(blob, { kind: "ArrayBuffer" });
    }

    /** @param {Blob} blob */
    readAsBinaryString(blob) {
      webidl.assertBranded(this, FileReaderPrototype);
      const prefix = "Failed to execute 'readAsBinaryString' on 'FileReader'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      // alias for readAsArrayBuffer
      this.#readOperation(blob, { kind: "BinaryString" });
    }

    /** @param {Blob} blob */
    readAsDataURL(blob) {
      webidl.assertBranded(this, FileReaderPrototype);
      const prefix = "Failed to execute 'readAsDataURL' on 'FileReader'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      // alias for readAsArrayBuffer
      this.#readOperation(blob, { kind: "DataUrl" });
    }

    /**
     * @param {Blob} blob
     * @param {string} [encoding]
     */
    readAsText(blob, encoding = undefined) {
      webidl.assertBranded(this, FileReaderPrototype);
      const prefix = "Failed to execute 'readAsText' on 'FileReader'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      if (encoding !== undefined) {
        encoding = webidl.converters["DOMString"](encoding, {
          prefix,
          context: "Argument 2",
        });
      }
      // alias for readAsArrayBuffer
      this.#readOperation(blob, { kind: "Text", encoding });
    }

    get onerror() {
      return this.#getEventHandlerFor("error");
    }
    set onerror(value) {
      this.#setEventHandlerFor("error", value);
    }

    get onloadstart() {
      return this.#getEventHandlerFor("loadstart");
    }
    set onloadstart(value) {
      this.#setEventHandlerFor("loadstart", value);
    }

    get onload() {
      return this.#getEventHandlerFor("load");
    }
    set onload(value) {
      this.#setEventHandlerFor("load", value);
    }

    get onloadend() {
      return this.#getEventHandlerFor("loadend");
    }
    set onloadend(value) {
      this.#setEventHandlerFor("loadend", value);
    }

    get onprogress() {
      return this.#getEventHandlerFor("progress");
    }
    set onprogress(value) {
      this.#setEventHandlerFor("progress", value);
    }

    get onabort() {
      return this.#getEventHandlerFor("abort");
    }
    set onabort(value) {
      this.#setEventHandlerFor("abort", value);
    }
  }

  webidl.configurePrototype(FileReader);
  const FileReaderPrototype = FileReader.prototype;

  ObjectDefineProperty(FileReader, "EMPTY", {
    writable: false,
    enumerable: true,
    configurable: false,
    value: 0,
  });
  ObjectDefineProperty(FileReader, "LOADING", {
    writable: false,
    enumerable: true,
    configurable: false,
    value: 1,
  });
  ObjectDefineProperty(FileReader, "DONE", {
    writable: false,
    enumerable: true,
    configurable: false,
    value: 2,
  });
  ObjectDefineProperty(FileReader.prototype, "EMPTY", {
    writable: false,
    enumerable: true,
    configurable: false,
    value: 0,
  });
  ObjectDefineProperty(FileReader.prototype, "LOADING", {
    writable: false,
    enumerable: true,
    configurable: false,
    value: 1,
  });
  ObjectDefineProperty(FileReader.prototype, "DONE", {
    writable: false,
    enumerable: true,
    configurable: false,
    value: 2,
  });

  function makeWrappedHandler(handler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      return FunctionPrototypeCall(
        wrappedHandler.handler,
        this,
        ...new SafeArrayIterator(args),
      );
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }

  window.__bootstrap.fileReader = {
    FileReader,
  };
})(this);
