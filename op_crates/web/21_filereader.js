// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const base64 = window.__bootstrap.base64;

  async function readOperation(fr, blob, readtype) {
    // Implementation from https://w3c.github.io/FileAPI/ notes
    // And body of deno blob.ts readBytes

    fr.aborting = false;

    // 1. If fr’s state is "loading", throw an InvalidStateError DOMException.
    if (fr.readyState === FileReader.LOADING) {
      throw new DOMException(
        "Invalid FileReader state.",
        "InvalidStateError",
      );
    }
    // 2. Set fr’s state to "loading".
    fr.readyState = FileReader.LOADING;
    // 3. Set fr’s result to null.
    fr.result = null;
    // 4. Set fr’s error to null.
    fr.error = null;

    // 5. Let stream be the result of calling get stream on blob.
    const stream /*: ReadableStream<ArrayBufferView>*/ = blob.stream();

    // 6. Let reader be the result of getting a reader from stream.
    const reader = stream.getReader();

    // 7. Let bytes be an empty byte sequence.
    //let bytes = new Uint8Array();
    const chunks /*: Uint8Array[]*/ = [];

    // 8. Let chunkPromise be the result of reading a chunk from stream with reader.
    let chunkPromise = reader.read();

    // 9. Let isFirstChunk be true.
    let isFirstChunk = true;

    // 10 in parallel while true
    while (!fr.aborting) {
      // 1. Wait for chunkPromise to be fulfilled or rejected.
      try {
        const chunk = await chunkPromise;

        // 2. If chunkPromise is fulfilled, and isFirstChunk is true, queue a task to fire a progress event called loadstart at fr.
        if (isFirstChunk) {
          queueMicrotask(() => {
            // fire a progress event for loadstart
            const ev = new ProgressEvent("loadstart", {});
            fr.dispatchEvent(ev);
          });
        }
        // 3. Set isFirstChunk to false.
        isFirstChunk = false;

        // 4. If chunkPromise is fulfilled with an object whose done property is false
        // and whose value property is a Uint8Array object, run these steps:
        if (!chunk.done && chunk.value instanceof Uint8Array) {
          chunks.push(chunk.value);

          // TODO(bartlomieju): (only) If roughly 50ms have passed since last progress
          {
            const size = chunks.reduce((p, i) => p + i.byteLength, 0);
            const ev = new ProgressEvent("progress", {
              loaded: size,
            });
            fr.dispatchEvent(ev);
          }

          chunkPromise = reader.read();
        } // 5 Otherwise, if chunkPromise is fulfilled with an object whose done property is true, queue a task to run the following steps and abort this algorithm:
        else if (chunk.done === true) {
          queueMicrotask(() => {
            if (fr.aborting) {
              return;
            }

            // 1. Set fr’s state to "done".
            fr.readyState = FileReader.DONE;
            // 2. Let result be the result of package data given bytes, type, blob’s type, and encodingName.
            const size = chunks.reduce((p, i) => p + i.byteLength, 0);
            const bytes = new Uint8Array(size);
            let offs = 0;
            for (const chunk of chunks) {
              bytes.set(chunk, offs);
              offs += chunk.byteLength;
            }
            switch (readtype.kind) {
              case "ArrayBuffer": {
                fr.result = bytes.buffer;
                break;
              }
              case "Text": {
                const decoder = new TextDecoder(readtype.encoding);
                fr.result = decoder.decode(bytes.buffer);
                break;
              }
              case "DataUrl": {
                fr.result = "data:application/octet-stream;base64," +
                  base64.fromByteArray(bytes);
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
              fr.dispatchEvent(ev);
            }

            // 5. If fr’s state is not "loading", fire a progress event called loadend at the fr.
            //Note: Event handler for the load or error events could have started another load, if that happens the loadend event for this load is not fired.
            if (fr.readyState !== FileReader.LOADING) {
              const ev = new ProgressEvent("loadend", {
                lengthComputable: true,
                loaded: size,
                total: size,
              });
              fr.dispatchEvent(ev);
            }
          });

          break;
        }
      } catch (err) {
        if (fr.aborting) {
          break;
        }

        // chunkPromise rejected
        fr.readyState = FileReader.DONE;
        fr.error = err;

        {
          const ev = new ProgressEvent("error", {});
          fr.dispatchEvent(ev);
        }

        //If fr’s state is not "loading", fire a progress event called loadend at fr.
        //Note: Event handler for the error event could have started another load, if that happens the loadend event for this load is not fired.
        if (fr.readyState !== FileReader.LOADING) {
          const ev = new ProgressEvent("loadend", {});
          fr.dispatchEvent(ev);
        }

        break;
      }
    }
  }

  class FileReader extends EventTarget {
    error = null;
    readyState = FileReader.EMPTY;
    result = null;
    aborting = false;

    constructor() {
      super();
    }

    abort() {
      // If context object's state is "empty" or if context object's state is "done" set context object's result to null and terminate this algorithm.
      if (
        this.readyState === FileReader.EMPTY ||
        this.readyState === FileReader.DONE
      ) {
        this.result = null;
        return;
      }
      // If context object's state is "loading" set context object's state to "done" and set context object's result to null.
      if (this.readyState === FileReader.LOADING) {
        this.readyState = FileReader.DONE;
        this.result = null;
      }
      // If there are any tasks from the context object on the file reading task source in an affiliated task queue, then remove those tasks from that task queue.
      // Terminate the algorithm for the read method being processed.
      this.aborting = true;

      // Fire a progress event called abort at the context object.
      const ev = new ProgressEvent("abort", {});
      this.dispatchEvent(ev);

      // If context object's state is not "loading", fire a progress event called loadend at the context object.
      if (this.readyState !== FileReader.LOADING) {
        const ev = new ProgressEvent("loadend", {});
        this.dispatchEvent(ev);
      }
    }
    readAsArrayBuffer(blob) {
      readOperation(this, blob, { kind: "ArrayBuffer" });
    }
    readAsBinaryString(blob) {
      // alias for readAsArrayBuffer
      readOperation(this, blob, { kind: "ArrayBuffer" });
    }
    readAsDataURL(blob) {
      readOperation(this, blob, { kind: "DataUrl" });
    }
    readAsText(blob, encoding) {
      readOperation(this, blob, { kind: "Text", encoding });
    }
  }

  FileReader.EMPTY = 0;
  FileReader.LOADING = 1;
  FileReader.DONE = 2;

  const handlerSymbol = Symbol("eventHandlers");

  function makeWrappedHandler(handler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      return wrappedHandler.handler.call(this, ...args);
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }
  // TODO(benjamingr) reuse when we can reuse code between web crates
  function defineEventHandler(emitter, name) {
    // HTML specification section 8.1.5.1
    Object.defineProperty(emitter, `on${name}`, {
      get() {
        return this[handlerSymbol]?.get(name)?.handler;
      },
      set(value) {
        if (!this[handlerSymbol]) {
          this[handlerSymbol] = new Map();
        }
        let handlerWrapper = this[handlerSymbol]?.get(name);
        if (handlerWrapper) {
          handlerWrapper.handler = value;
        } else {
          handlerWrapper = makeWrappedHandler(value);
          this.addEventListener(name, handlerWrapper);
        }
        this[handlerSymbol].set(name, handlerWrapper);
      },
      configurable: true,
      enumerable: true,
    });
  }
  defineEventHandler(FileReader.prototype, "error");
  defineEventHandler(FileReader.prototype, "loadstart");
  defineEventHandler(FileReader.prototype, "load");
  defineEventHandler(FileReader.prototype, "loadend");
  defineEventHandler(FileReader.prototype, "progress");
  defineEventHandler(FileReader.prototype, "abort");

  window.__bootstrap.fileReader = {
    FileReader,
  };
})(this);
