// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/globals.ts",
  [
    "./lib.deno.shared_globals.d.ts",
    "$deno$/web/blob.ts",
    "$deno$/web/console.ts",
    "$deno$/web/custom_event.ts",
    "$deno$/web/dom_exception.ts",
    "$deno$/web/dom_file.ts",
    "$deno$/web/event.ts",
    "$deno$/web/event_target.ts",
    "$deno$/web/form_data.ts",
    "$deno$/web/fetch.ts",
    "$deno$/web/headers.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/web/timers.ts",
    "$deno$/web/url.ts",
    "$deno$/web/url_search_params.ts",
    "$deno$/web/workers.ts",
    "$deno$/web/performance.ts",
    "$deno$/web/request.ts",
    "$deno$/web/streams/mod.ts",
    "$deno$/core.ts",
  ],
  function (exports_102, context_102) {
    "use strict";
    let blob,
      consoleTypes,
      customEvent,
      domException,
      domFile,
      event,
      eventTarget,
      formData,
      fetchTypes,
      headers,
      textEncoding,
      timers,
      url,
      urlSearchParams,
      workers,
      performanceUtil,
      request,
      streams,
      core_ts_7;
    const __moduleName = context_102 && context_102.id;
    function writable(value) {
      return {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
      };
    }
    exports_102("writable", writable);
    function nonEnumerable(value) {
      return {
        value,
        writable: true,
        configurable: true,
      };
    }
    exports_102("nonEnumerable", nonEnumerable);
    function readOnly(value) {
      return {
        value,
        enumerable: true,
      };
    }
    exports_102("readOnly", readOnly);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function getterOnly(getter) {
      return {
        get: getter,
        enumerable: true,
      };
    }
    exports_102("getterOnly", getterOnly);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function setEventTargetData(value) {
      eventTarget.eventTargetData.set(
        value,
        eventTarget.getDefaultTargetData()
      );
    }
    exports_102("setEventTargetData", setEventTargetData);
    return {
      setters: [
        function (_1) {},
        function (blob_4) {
          blob = blob_4;
        },
        function (consoleTypes_1) {
          consoleTypes = consoleTypes_1;
        },
        function (customEvent_1) {
          customEvent = customEvent_1;
        },
        function (domException_1) {
          domException = domException_1;
        },
        function (domFile_2) {
          domFile = domFile_2;
        },
        function (event_1) {
          event = event_1;
        },
        function (eventTarget_1) {
          eventTarget = eventTarget_1;
        },
        function (formData_1) {
          formData = formData_1;
        },
        function (fetchTypes_1) {
          fetchTypes = fetchTypes_1;
        },
        function (headers_1) {
          headers = headers_1;
        },
        function (textEncoding_1) {
          textEncoding = textEncoding_1;
        },
        function (timers_1) {
          timers = timers_1;
        },
        function (url_1) {
          url = url_1;
        },
        function (urlSearchParams_1) {
          urlSearchParams = urlSearchParams_1;
        },
        function (workers_1) {
          workers = workers_1;
        },
        function (performanceUtil_1) {
          performanceUtil = performanceUtil_1;
        },
        function (request_1) {
          request = request_1;
        },
        function (streams_2) {
          streams = streams_2;
        },
        function (core_ts_7_1) {
          core_ts_7 = core_ts_7_1;
        },
      ],
      execute: function () {
        // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
        exports_102("windowOrWorkerGlobalScopeMethods", {
          atob: writable(textEncoding.atob),
          btoa: writable(textEncoding.btoa),
          clearInterval: writable(timers.clearInterval),
          clearTimeout: writable(timers.clearTimeout),
          fetch: writable(fetchTypes.fetch),
          // queueMicrotask is bound in Rust
          setInterval: writable(timers.setInterval),
          setTimeout: writable(timers.setTimeout),
        });
        // Other properties shared between WindowScope and WorkerGlobalScope
        exports_102("windowOrWorkerGlobalScopeProperties", {
          console: writable(new consoleTypes.Console(core_ts_7.core.print)),
          Blob: nonEnumerable(blob.DenoBlob),
          File: nonEnumerable(domFile.DomFileImpl),
          CustomEvent: nonEnumerable(customEvent.CustomEventImpl),
          DOMException: nonEnumerable(domException.DOMExceptionImpl),
          Event: nonEnumerable(event.EventImpl),
          EventTarget: nonEnumerable(eventTarget.EventTargetImpl),
          URL: nonEnumerable(url.URLImpl),
          URLSearchParams: nonEnumerable(urlSearchParams.URLSearchParamsImpl),
          Headers: nonEnumerable(headers.HeadersImpl),
          FormData: nonEnumerable(formData.FormDataImpl),
          TextEncoder: nonEnumerable(textEncoding.TextEncoder),
          TextDecoder: nonEnumerable(textEncoding.TextDecoder),
          ReadableStream: nonEnumerable(streams.ReadableStream),
          Request: nonEnumerable(request.Request),
          Response: nonEnumerable(fetchTypes.Response),
          performance: writable(new performanceUtil.Performance()),
          Worker: nonEnumerable(workers.WorkerImpl),
        });
        exports_102("eventTargetProperties", {
          addEventListener: readOnly(
            eventTarget.EventTargetImpl.prototype.addEventListener
          ),
          dispatchEvent: readOnly(
            eventTarget.EventTargetImpl.prototype.dispatchEvent
          ),
          removeEventListener: readOnly(
            eventTarget.EventTargetImpl.prototype.removeEventListener
          ),
        });
      },
    };
  }
);
