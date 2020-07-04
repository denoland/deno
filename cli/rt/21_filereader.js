// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const base64 = window.__bootstrap.base64;

  class FileReader extends EventTarget {
    error = null;
    onabort = null;
    onerror = null;
    onload = null;
    onloadend = null;
    onloadstart = null;
    onprogress = null;

    readyState = FileReader.EMPTY;
    result = null;

    constructor(
    ) {
      super();
    }

    abort() {
      // not implemented
    }
    readAsArrayBuffer(blob) {
      this.#readOperation(blob, {kind: 'ArrayBuffer'});
    }
    readAsBinaryString(blob) {
      // alias for readAsArrayBuffer
      this.#readOperation(blob, {kind: 'ArrayBuffer'});
    }
    readAsDataURL(blob) {
      this.#readOperation(blob, {kind: 'DataUrl'});
    }
    readAsText(blob, encoding) {
      this.#readOperation(blob, {kind: 'Text', encoding});
    }

    async #readOperation(blob, readtype) {
      // Implementation from https://w3c.github.io/FileAPI/ notes
      // And body of deno blob.ts readBytes

      // 1. If fr’s state is "loading", throw an InvalidStateError DOMException.
      if(this.readyState === FileReader.LOADING) {
        throw new DOMExcechunkPromiseption("Invalid FileReader state.", "InvalidStateError");
      }
      // 2. Set fr’s state to "loading".
      this.readyState = FileReader.LOADING;
      // 3. Set fr’s result to null.
      this.result = null;
      // 4. Set fr’s error to null.
      this.error = null;

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
      while (true) {
        // 1. Wait for chunkPromise to be fulfilled or rejected.
        try {
          const chunk = await chunkPromise;

          // 2. If chunkPromise is fulfilled, and isFirstChunk is true, queue a task to fire a progress event called loadstart at fr.
          if(isFirstChunk) {
            setTimeout(()=>{
              // fire a progress event for loadstart
              const ev = new ProgressEvent('loadstart', {
              });
              this.dispatchEvent(ev);
              if(this.onloadstart!==null) {
                this.onloadstart(ev);
              }
            },0);
          }
          // 3. Set isFirstChunk to false.
          isFirstChunk = false;

          // 4. If chunkPromise is fulfilled with an object whose done property is false
          // and whose value property is a Uint8Array object, run these steps:
          if (!chunk.done && chunk.value instanceof Uint8Array) {
            chunks.push(chunk.value);

            // TODO: (only) If roughly 50ms have passed since last progress
            {
              const size = chunks.reduce((p, i) => p + i.byteLength, 0);
              const ev = new ProgressEvent('progress', {
                loaded: size
              });
              this.dispatchEvent(ev);
              if(this.onprogress!==null) {
                this.onprogress(ev);
              }
            }

            chunkPromise = reader.read();
          }
          // 5 Otherwise, if chunkPromise is fulfilled with an object whose done property is true, queue a task to run the following steps and abort this algorithm:
          else if( chunk.done === true ) {
            setTimeout(()=>{
              // 1. Set fr’s state to "done".
              this.readyState = FileReader.DONE;
              // 2. Let result be the result of package data given bytes, type, blob’s type, and encodingName.
              const size = chunks.reduce((p, i) => p + i.byteLength, 0);
              const bytes = new Uint8Array(size);
              let offs = 0;
              for (const chunk of chunks) {
                bytes.set(chunk, offs);
                offs += chunk.byteLength;
              }
              switch(readtype.kind) {
                case 'ArrayBuffer': {
                  this.result = bytes.buffer;
                  break
                }
                case 'Text': {
                  const decoder = new TextDecoder(readtype.encoding);
                  this.result = decoder.decode(bytes.buffer);
                  break;
                }
                case 'DataUrl': {
                  this.result = "data:application/octet-stream;base64," + base64.fromByteArray(bytes);
                  break;
                }
              }
              // 4.2 Fire a progress event called load at the fr.
              {
                const ev = new ProgressEvent('load', {
                  lengthComputable: true,
                  loaded: size,
                  total: size
                });
                this.dispatchEvent(ev);
                if(this.onload!==null) {
                  this.onload(ev);
                }
              }


              // 5. If fr’s state is not "loading", fire a progress event called loadend at the fr.
              //Note: Event handler for the load or error events could have started another load, if that happens the loadend event for this load is not fired.
              if(this.readyState !== FileReader.LOADING) {
                const ev = new ProgressEvent('loadend', {
                  lengthComputable: true,
                  loaded: size,
                  total: size
                });
                this.dispatchEvent(ev);
                if(this.onloadend!==null) {
                  this.onloadend(ev);
                }
              }
            },0);

            break;
          }
        }
        catch(err) {
          // chunkPromise rejected
          this.readyState = FileReader.DONE;
          this.error = err;

          {
            const ev = new ProgressEvent('error', {
            });
            this.dispatchEvent(ev);
            if(this.onerror!==null) {
              this.onerror(ev);
            }
          }

          //If fr’s state is not "loading", fire a progress event called loadend at fr.
          //Note: Event handler for the error event could have started another load, if that happens the loadend event for this load is not fired.
          if(this.readyState !== FileReader.LOADING) {
            const ev = new ProgressEvent('loadend', {
            });
            this.dispatchEvent(ev);
            if(this.onloadend!==null) {
              this.onloadend(ev);
            }
          }

          break;
        }
      }
    }
  };

  FileReader.EMPTY = 0;
  FileReader.LOADING = 1;
  FileReader.DONE = 2;

  window.__bootstrap.fileReader = {
    FileReader,
  };
})(this);
