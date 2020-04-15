System.register(
  "$deno$/web/workers.ts",
  [
    "$deno$/ops/worker_host.ts",
    "$deno$/util.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/web/event.ts",
    "$deno$/web/event_target.ts",
  ],
  function (exports_98, context_98) {
    "use strict";
    let worker_host_ts_1,
      util_ts_20,
      text_encoding_ts_8,
      event_ts_3,
      event_target_ts_1,
      encoder,
      decoder,
      MessageEvent,
      ErrorEvent,
      WorkerImpl;
    const __moduleName = context_98 && context_98.id;
    function encodeMessage(data) {
      const dataJson = JSON.stringify(data);
      return encoder.encode(dataJson);
    }
    function decodeMessage(dataIntArray) {
      const dataJson = decoder.decode(dataIntArray);
      return JSON.parse(dataJson);
    }
    return {
      setters: [
        function (worker_host_ts_1_1) {
          worker_host_ts_1 = worker_host_ts_1_1;
        },
        function (util_ts_20_1) {
          util_ts_20 = util_ts_20_1;
        },
        function (text_encoding_ts_8_1) {
          text_encoding_ts_8 = text_encoding_ts_8_1;
        },
        function (event_ts_3_1) {
          event_ts_3 = event_ts_3_1;
        },
        function (event_target_ts_1_1) {
          event_target_ts_1 = event_target_ts_1_1;
        },
      ],
      execute: function () {
        encoder = new text_encoding_ts_8.TextEncoder();
        decoder = new text_encoding_ts_8.TextDecoder();
        MessageEvent = class MessageEvent extends event_ts_3.EventImpl {
          constructor(type, eventInitDict) {
            super(type, {
              bubbles: eventInitDict?.bubbles ?? false,
              cancelable: eventInitDict?.cancelable ?? false,
              composed: eventInitDict?.composed ?? false,
            });
            this.data = eventInitDict?.data ?? null;
            this.origin = eventInitDict?.origin ?? "";
            this.lastEventId = eventInitDict?.lastEventId ?? "";
          }
        };
        exports_98("MessageEvent", MessageEvent);
        ErrorEvent = class ErrorEvent extends event_ts_3.EventImpl {
          constructor(type, eventInitDict) {
            super(type, {
              bubbles: eventInitDict?.bubbles ?? false,
              cancelable: eventInitDict?.cancelable ?? false,
              composed: eventInitDict?.composed ?? false,
            });
            this.message = eventInitDict?.message ?? "";
            this.filename = eventInitDict?.filename ?? "";
            this.lineno = eventInitDict?.lineno ?? 0;
            this.colno = eventInitDict?.colno ?? 0;
            this.error = eventInitDict?.error ?? null;
          }
        };
        exports_98("ErrorEvent", ErrorEvent);
        WorkerImpl = class WorkerImpl extends event_target_ts_1.EventTargetImpl {
          constructor(specifier, options) {
            super();
            this.#terminated = false;
            this.#handleMessage = (msgData) => {
              let data;
              try {
                data = decodeMessage(new Uint8Array(msgData));
              } catch (e) {
                const msgErrorEvent = new MessageEvent("messageerror", {
                  cancelable: false,
                  data,
                });
                if (this.onmessageerror) {
                  this.onmessageerror(msgErrorEvent);
                }
                return;
              }
              const msgEvent = new MessageEvent("message", {
                cancelable: false,
                data,
              });
              if (this.onmessage) {
                this.onmessage(msgEvent);
              }
              this.dispatchEvent(msgEvent);
            };
            this.#handleError = (e) => {
              const event = new ErrorEvent("error", {
                cancelable: true,
                message: e.message,
                lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
                colno: e.columnNumber ? e.columnNumber + 1 : undefined,
                filename: e.fileName,
                error: null,
              });
              let handled = false;
              if (this.onerror) {
                this.onerror(event);
              }
              this.dispatchEvent(event);
              if (event.defaultPrevented) {
                handled = true;
              }
              return handled;
            };
            this.#poll = async () => {
              while (!this.#terminated) {
                const event = await worker_host_ts_1.hostGetMessage(this.#id);
                // If terminate was called then we ignore all messages
                if (this.#terminated) {
                  return;
                }
                const type = event.type;
                if (type === "terminalError") {
                  this.#terminated = true;
                  if (!this.#handleError(event.error)) {
                    throw Error(event.error.message);
                  }
                  continue;
                }
                if (type === "msg") {
                  this.#handleMessage(event.data);
                  continue;
                }
                if (type === "error") {
                  if (!this.#handleError(event.error)) {
                    throw Error(event.error.message);
                  }
                  continue;
                }
                if (type === "close") {
                  util_ts_20.log(
                    `Host got "close" message from worker: ${this.#name}`
                  );
                  this.#terminated = true;
                  return;
                }
                throw new Error(`Unknown worker event: "${type}"`);
              }
            };
            const { type = "classic", name = "unknown" } = options ?? {};
            if (type !== "module") {
              throw new Error(
                'Not yet implemented: only "module" type workers are supported'
              );
            }
            this.#name = name;
            const hasSourceCode = false;
            const sourceCode = decoder.decode(new Uint8Array());
            /* TODO(bartlomieju):
                    // Handle blob URL.
                    if (specifier.startsWith("blob:")) {
                      hasSourceCode = true;
                      const b = blobURLMap.get(specifier);
                      if (!b) {
                        throw new Error("No Blob associated with the given URL is found");
                      }
                      const blobBytes = blobBytesWeakMap.get(b!);
                      if (!blobBytes) {
                        throw new Error("Invalid Blob");
                      }
                      sourceCode = blobBytes!;
                    }
                    */
            const { id } = worker_host_ts_1.createWorker(
              specifier,
              hasSourceCode,
              sourceCode,
              options?.name
            );
            this.#id = id;
            this.#poll();
          }
          #id;
          #name;
          #terminated;
          #handleMessage;
          #handleError;
          #poll;
          postMessage(message, transferOrOptions) {
            if (transferOrOptions) {
              throw new Error(
                "Not yet implemented: `transfer` and `options` are not supported."
              );
            }
            if (this.#terminated) {
              return;
            }
            worker_host_ts_1.hostPostMessage(this.#id, encodeMessage(message));
          }
          terminate() {
            if (!this.#terminated) {
              this.#terminated = true;
              worker_host_ts_1.hostTerminateWorker(this.#id);
            }
          }
        };
        exports_98("WorkerImpl", WorkerImpl);
      },
    };
  }
);
