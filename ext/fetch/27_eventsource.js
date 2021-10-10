// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// Refer https://github.com/MierenManz/EventSource
((window) => {
  const { webidl } = window.__bootstrap;
  const { DOMException } = window.__bootstrap.domException;
  const { URL } = window.__bootstrap.url;
  const { EventTarget } = window.__bootstrap.eventTarget;
  const {
    ObjectDefineProperties,
    Symbol,
    ArrayPrototypeSome,
    Promise,
    isNaN,
    isFinite,
    Number,
    StringPrototypeTrim,
    StringPrototypeSlice,
    StringPrototypeReplaceAll,
    StringPrototypeSplit,
    StringPrototypeIncludes,
    ArrayPrototypePop,
    StringPrototypeIndexOf,
    setTimeout,
    clearTimeout,
    decodeURIComponent,
  } = window.__bootstrap.primordials;
  const { getLocationHref } = window.__bootstrap.location;
  const { TextDecoderStream } = window.__bootstrap.encoding;
  const { mainFetch, newInnerRequest } = window.__bootstrap.fetch;
  const { defineEventHandler } = window.__bootstrap.event;

  webidl.converters["EventSourceInit"] = webidl.createDictionaryConverter(
    "EventSourceInit",
    [
      {
        key: "withCredentials",
        converter: webidl.converters.boolean,
        required: true,
      },
    ],
  );

  const CONNECTING = 0;
  const OPEN = 1;
  const CLOSED = 2;
  const defaultHeaders = [
    ["Accept", "text/event-stream"],
    ["Cache-Control", "no-store"],
  ];

  const _readyState = Symbol("readystate");
  const _withCredentials = Symbol("withcredentials");
  const _abortSignal = Symbol("abortsignal");
  const _url = Symbol("url");
  const _fetchHeaders = Symbol("fetchheaders");
  const _lastEventID = Symbol("lasteventid");
  const _reconnectionTime = Symbol("reconnetiontime");
  const _fetch = Symbol("fetch");

  class EventSource extends EventTarget {
    [_readyState] = CONNECTING;
    [_withCredentials] = false;
    [_abortSignal] = new AbortController();
    [_fetchHeaders] = defaultHeaders;
    [_lastEventID] = "";
    [_reconnectionTime] = 2200;
    get CONNECTING() {
      webidl.assertBranded(this, EventSource);
      return CONNECTING;
    }

    get OPEN() {
      webidl.assertBranded(this, EventSource);
      return OPEN;
    }

    get CLOSED() {
      webidl.assertBranded(this, EventSource);
      return CLOSED;
    }

    get readyState() {
      webidl.assertBranded(this, EventSource);
      return this[_readyState];
    }

    get url() {
      webidl.assertBranded(this, EventSource);
      return this[_url];
    }

    get withCredentials() {
      webidl.assertBranded(this, EventSource);
      return this[_withCredentials];
    }

    constructor(url, eventSourceInitDict) {
      super();

      const prefix = "Failed to construct 'EventSource'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      if (eventSourceInitDict) {
        eventSourceInitDict = webidl.converters["EventSourceInit"]({
          prefix,
          context: "Argument 2",
        });
      }

      try {
        // Allow empty url
        // https://github.com/web-platform-tests/wpt/blob/master/eventsource/eventsource-constructor-empty-url.any.js
        const baseURL = getLocationHref();
        this[_url] = new URL(url, baseURL).href;
      } catch (e) {
        throw new DOMException(e.message, "SyntaxError");
      }

      if (eventSourceInitDict?.withCredentials) {
        this[_withCredentials] = true;
      }

      this[_fetch]();
      return;
    }

    close() {
      // Why does this error.
      // webidl.assertBranded(this, EventSource);
      this[_readyState] = CLOSED;
      this[_abortSignal].abort();
    }

    async [_fetch]() {
      let currentRetries = 0;
      while (this[_readyState] < CLOSED) {
        const req = newInnerRequest("GET", this[_url], this[_fetchHeaders], null);
        /** @type { InnerResponse } */
        const res = await mainFetch(req, true, this[_abortSignal].signal);
        const correctContentType = ArrayPrototypeSome(res.headerList, (header) =>
          header[0] === "content-type" && header[1] === "text/event-stream"
        );
        if (
          res?.body &&
          res?.status === 200 &&
          correctContentType
        ) {
          // Announce connection
          if (this[_readyState] !== CLOSED) {
            this[_readyState] = OPEN;
            const openEvent = new Event("open", {
              bubbles: false,
              cancelable: false,
            });
            super.dispatchEvent(openEvent);
            this.onopen?.(openEvent);
          }

          // Decode body for interpreting
          const decoder = new TextDecoderStream("utf-8", {
            ignoreBOM: false,
            fatal: false,
          });
          const reader = res.body.stream.pipeThrough(decoder);

          // Initiate buffers
          let lastEventIDBuffer = "";
          let eventTypeBuffer = "";
          let messageBuffer = "";
          let readBuffer = "";

          for await (const chunk of reader) {
            if (this[_abortSignal].signal.aborted) break;
            const lines = StringPrototypeSplit(
              StringPrototypeReplaceAll(
                StringPrototypeReplaceAll(
                  decodeURIComponent(readBuffer + chunk),
                  "\r\n",
                  "\n",
                ),
                "\r",
                "\n",
              ),
              "\n",
            );
            readBuffer = ArrayPrototypePop(lines) ?? "";

            // Start loop for interpreting
            for (const line of lines) {
              if (!line) {
                this[_lastEventID] = lastEventIDBuffer;

                // Check if buffer is not an empty string
                if (messageBuffer) {
                  // Create event
                  if (!eventTypeBuffer) {
                    eventTypeBuffer = "message";
                  }

                  const event = new MessageEvent(eventTypeBuffer, {
                    data: StringPrototypeTrim(messageBuffer),
                    origin: res.url,
                    lastEventId: this[_lastEventID],
                    cancelable: false,
                    bubbles: false,
                  });

                  if (this[_readyState] !== CLOSED) {
                    // Fire event
                    super.dispatchEvent(event);
                    this.onmessage?.(event);
                  }
                }

                // Clear buffers
                messageBuffer = "";
                eventTypeBuffer = "";
                continue;
              }

              // Ignore comments
              if (line[0] === ":") continue;

              let splitIndex = StringPrototypeIndexOf(line, ":");
              splitIndex = splitIndex > 0 ? splitIndex : line.length;
              const field = StringPrototypeTrim(
                StringPrototypeSlice(line, 0, splitIndex),
              );
              /** @type { string } */
              const data = StringPrototypeTrim(
                StringPrototypeSlice(line, splitIndex + 1),
              );
              switch (field) {
                case "event":
                  // Set fieldBuffer to Field Value
                  eventTypeBuffer = data;
                  break;
                case "data":
                  // append Field Value to dataBuffer
                  messageBuffer += `${data}\n`;
                  break;
                case "id":
                  // set lastEventID to Field Value
                  if (
                    data && !StringPrototypeIncludes(data, "\u0000") &&
                    !StringPrototypeIncludes(data, "\x00")
                  ) {
                    lastEventIDBuffer = data;
                  }
                  break;
                case "retry": {
                  // set reconnectionTime to Field Value if int
                  const num = Number(data);
                  if (!isNaN(num) && isFinite(num)) {
                    this[_reconnectionTime] = num;
                  }
                  break;
                }
              }
            }
          }
          if (this[_abortSignal].signal.aborted) {
            // Cancel reader to close the EventSource properly
            await reader.cancel();
            this[_readyState] = CLOSED;
            break;
          }
        } else {
          // Connection failed for whatever reason
          this[_readyState] = CLOSED;
          this[_abortSignal].abort();
          const errorEvent = new Event("error", {
            bubbles: false,
            cancelable: false,
          });
          super.dispatchEvent(errorEvent);
          this.onerror?.(errorEvent);
          if (currentRetries >= 3) break;
          currentRetries++;
        }

        // Set readyState to CONNECTING
        if (this[_readyState] !== CLOSED) {
          this[_readyState] = CONNECTING;

          // Fire onerror
          const errorEvent = new Event("error", {
            bubbles: false,
            cancelable: false,
          });

          super.dispatchEvent(errorEvent);
          this.onerror?.(errorEvent);

          // Timeout for re-establishing the connection
          await new Promise((res) => {
            const id = setTimeout(
              () => res(clearTimeout(id)),
              this[_reconnectionTime],
            );
          });

          if (this[_readyState] !== CONNECTING) break;

          if (this[_lastEventID]) {
            this[_fetchHeaders].push([
              "Last-Event-ID",
              this[_lastEventID],
            ]);
          }
        }
      }
    }
  }

  ObjectDefineProperties(EventSource, {
    CONNECTING: {
      value: 0,
    },
    OPEN: {
      value: 1,
    },
    CLOSED: {
      value: 2,
    },
  });

  defineEventHandler(EventSource.prototype, "message", null);
  defineEventHandler(EventSource.prototype, "error", null);
  defineEventHandler(EventSource.prototype, "open", null);
  webidl.configurePrototype(EventSource);

  window.__bootstrap.eventSource = EventSource;
})(this);
