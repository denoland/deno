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
    StringPrototypeTrim,
    Number,
    NumberIsNaN,
    NumberIsFinite,
    StringPrototypeSlice,
    StringPrototypeReplaceAll,
    StringPrototypeSplit,
    StringPrototypeIncludes,
    StringPrototypeToLowerCase,
    ArrayPrototypePop,
    ArrayPrototypePush,
    StringPrototypeIndexOf,
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
        defaultValue: false,
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
    #withCredentials = false;
    #readyState = 0;
    #abortController = new AbortController();
    #settings = {
      url: "",
      fetchSettings: {
        headers: defaultHeaders,
        credentials: "same-origin",
        mode: "cors",
      },
      reconnectionTime: 2200,
      lastEventID: "",
    };

    onopen = null;
    onmessage = null;
    onerror = null;

    CONNECTING = 0;
    OPEN = 1;
    CLOSED = 2;

    get readyState() {
      return this.#readyState;
    }

    get url() {
      return this.#settings.url;
    }

    get withCredentials() {
      return this.#withCredentials;
    }

    constructor(url, eventSourceInitDict) {
      super();
      this[webidl.brand] = webidl.brand;
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
        this.#settings.url = new URL(url, baseURL).href;
      } catch (e) {
        throw new DOMException(e.message, "SyntaxError");
      }

      if (eventSourceInitDict?.withCredentials) {
        this.#settings.fetchSettings.credentials = "include";
        this.#withCredentials = true;
      }

      this.#fetch();
      return;
    }

    close() {
      this.#readyState = this.CLOSED;
      this.#abortController.abort();
    }

    async #fetch() {
      let currentRetries = 0;
      while (this.#readyState < this.CLOSED) {
        const req = newInnerRequest(
          "GET",
          this.url,
          this.#settings.fetchSettings.headers,
          null,
        );
        /** @type { InnerResponse } */
        const res = await mainFetch(req, true, this.#abortController.signal);
        const correctContentType = ArrayPrototypeSome(
          res.headerList,
          (header) =>
            StringPrototypeToLowerCase(header[0]) === "content-type" &&
            StringPrototypeIncludes(header[1], "text/event-stream"),
        );
        if (
          res?.body &&
          res?.status === 200 &&
          correctContentType
        ) {
          // Announce connection
          if (this.#readyState !== this.CLOSED) {
            this.#readyState = this.OPEN;
            const openEvent = new Event("open", {
              bubbles: false,
              cancelable: false,
            });
            super.dispatchEvent(openEvent);
            if (this.onopen) this.onopen(openEvent);
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
            if (this.#abortController.signal.aborted) break;
            const lines = decodeURIComponent(readBuffer + chunk)
              .replaceAll("\r\n", "\n")
              .replaceAll("\r", "\n")
              .split("\n");
            readBuffer = lines.pop() ?? "";

            // Start loop for interpreting
            for (const line of lines) {
              if (!line) {
                this.#settings.lastEventID = lastEventIDBuffer;

                // Check if buffer is not an empty string
                if (messageBuffer) {
                  // Create event
                  if (!eventTypeBuffer) {
                    eventTypeBuffer = "message";
                  }

                  const event = new MessageEvent(eventTypeBuffer, {
                    data: messageBuffer.trim(),
                    origin: res.url,
                    lastEventId: this.#settings.lastEventID,
                    cancelable: false,
                    bubbles: false,
                  });

                  if (this.readyState !== this.CLOSED) {
                    // Fire event
                    super.dispatchEvent(event);
                    if (this.onmessage) this.onmessage(event);
                  }
                }

                // Clear buffers
                messageBuffer = "";
                eventTypeBuffer = "";
                continue;
              }

              // Ignore comments
              if (line[0] === ":") continue;

              let splitIndex = line.indexOf(":");
              splitIndex = splitIndex > 0 ? splitIndex : line.length;
              const field = line.slice(0, splitIndex).trim();
              const data = line.slice(splitIndex + 1).trim();
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
                    data && !data.includes("\u0000") && !data.includes("\x00")
                  ) {
                    lastEventIDBuffer = data;
                  }
                  break;
                case "retry": {
                  // set reconnectionTime to Field Value if int
                  const num = Number(data);
                  if (!isNaN(num) && isFinite(num)) {
                    this.#settings.reconnectionTime = num;
                  }
                  break;
                }
              }
            }
          }
          if (this.#abortController.signal.aborted) {
            // Cancel reader to close the EventSource properly
            await reader.cancel();
            this.#readyState = this.CLOSED;
            break;
          }
        } else {
          // Connection failed for whatever reason
          this.#readyState = this.CLOSED;
          this.#abortController.abort();
          const errorEvent = new Event("error", {
            bubbles: false,
            cancelable: false,
          });
          super.dispatchEvent(errorEvent);
          if (this.onerror) this.onerror(errorEvent);
          if (currentRetries >= 3) break;
          currentRetries++;
        }

        // Set readyState to CONNECTING
        if (this.#readyState !== this.CLOSED) {
          this.#readyState = this.CONNECTING;

          // Fire onerror
          const errorEvent = new Event("error", {
            bubbles: false,
            cancelable: false,
          });

          super.dispatchEvent(errorEvent);
          if (this.onerror) this.onerror(errorEvent);

          // Timeout for re-establishing the connection
          await new Promise((res) => {
            const id = setTimeout(
              () => res(clearTimeout(id)),
              this.#settings.reconnectionTime,
            );
          });

          if (this.#readyState !== this.CONNECTING) break;

          if (this.#settings.lastEventID) {
            this.#settings.fetchSettings.headers.push([
              "Last-Event-ID",
              this.#settings.lastEventID,
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
  const EventSourcePrototype = EventSource.prototype;

  window.__bootstrap.fetch ??= {};
  window.__bootstrap.fetch.EventSource = EventSource;
})(this);
