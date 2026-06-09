# deno web

**Implements timers, as well as the following APIs:**

- Event
- TextEncoder
- TextDecoder
- File (Spec: https://w3c.github.io/FileAPI)

_Note: Testing for text encoding is done via WPT in cli/._

## Usage Example

From javascript, include the extension's source:

```javascript
import { core } from "ext:core/mod.js";

const infra = core.loadExtScript("ext:deno_web/00_infra.js");
const url = core.loadExtScript("ext:deno_web/00_url.js");
const broadcastChannel = core.loadExtScript(
  "ext:deno_web/01_broadcast_channel.js",
);
const console = core.loadExtScript("ext:deno_web/01_console.js");
const DOMException = core.loadExtScript("ext:deno_web/01_dom_exception.js");
const mimesniff = core.loadExtScript("ext:deno_web/01_mimesniff.js");
const urlPattern = core.loadExtScript("ext:deno_web/01_urlpattern.js");
const event = core.loadExtScript("ext:deno_web/02_event.js");
const structuredClone = core.loadExtScript(
  "ext:deno_web/02_structured_clone.js",
);
const timers = core.loadExtScript("ext:deno_web/02_timers.js");
const abortSignal = core.loadExtScript("ext:deno_web/03_abort_signal.js");
const globalInterfaces = core.loadExtScript(
  "ext:deno_web/04_global_interfaces.js",
);
const base64 = core.loadExtScript("ext:deno_web/05_base64.js");
const streams = core.loadExtScript("ext:deno_web/06_streams.js");
const encoding = core.loadExtScript("ext:deno_web/08_text_encoding.js");
const file = core.loadExtScript("ext:deno_web/09_file.js");
const fileReader = core.loadExtScript("ext:deno_web/10_filereader.js");
const location = core.loadExtScript("ext:deno_web/12_location.js");
const messagePort = core.loadExtScript("ext:deno_web/13_message_port.js");
const compression = core.loadExtScript("ext:deno_web/14_compression.js");
const performance = core.loadExtScript("ext:deno_web/15_performance.js");
const imageData = core.loadExtScript("ext:deno_web/16_image_data.js");
const loadGeometry = core.createLazyLoader("ext:deno_web/geometry.js");
const loadWebTransport = core.createLazyLoader("ext:deno_web/webtransport.js");
const geometry = loadGeometry();
const webTransport = loadWebTransport();
```

Then assign the properties below to the global scope like this example:

```javascript
Object.defineProperty(globalThis, "AbortController", {
  value: abortSignal.AbortController,
  enumerable: false,
  configurable: true,
  writable: true,
});
```

| Name                             | Value                                    | enumerable | configurable | writeable |
| -------------------------------- | ---------------------------------------- | ---------- | ------------ | --------- |
| AbortController                  | abortSignal.AbortController              | false      | true         | true      |
| AbortSignal                      | abortSignal.AbortSignal                  | false      | true         | true      |
| Blob                             | file.Blob                                | false      | true         | true      |
| BroadcastChannel                 | broadcastChannel.BroadcastChannel        | false      | true         | true      |
| ByteLengthQueuingStrategy        | streams.ByteLengthQueuingStrategy        |            |              |           |
| CloseEvent                       | event.CloseEvent                         | false      | true         | true      |
| CompressionStream                | compression.CompressionStream            | false      | true         | true      |
| CountQueuingStrategy             | streams.CountQueuingStrategy             |            |              |           |
| CustomEvent                      | event.CustomEvent                        | false      | true         | true      |
| DecompressionStream              | compression.DecompressionStream          | false      | true         | true      |
| DOMException                     | DOMException                             | false      | true         | true      |
| ErrorEvent                       | event.ErrorEvent                         | false      | true         | true      |
| Event                            | event.Event                              | false      | true         | true      |
| EventTarget                      | event.EventTarget                        | false      | true         | true      |
| File                             | file.File                                | false      | true         | true      |
| FileReader                       | fileReader.FileReader                    | false      | true         | true      |
| MessageEvent                     | event.MessageEvent                       | false      | true         | true      |
| Performance                      | performance.Performance                  | false      | true         | true      |
| PerformanceEntry                 | performance.PerformanceEntry             | false      | true         | true      |
| PerformanceMark                  | performance.PerformanceMark              | false      | true         | true      |
| PerformanceMeasure               | performance.PerformanceMeasure           | false      | true         | true      |
| PromiseRejectionEvent            | event.PromiseRejectionEvent              | false      | true         | true      |
| ProgressEvent                    | event.ProgressEvent                      | false      | true         | true      |
| ReadableStream                   | streams.ReadableStream                   | false      | true         | true      |
| ReadableStreamDefaultReader      | streams.ReadableStreamDefaultReader      |            |              |           |
| TextDecoder                      | encoding.TextDecoder                     | false      | true         | true      |
| TextEncoder                      | encoding.TextEncoder                     | false      | true         | true      |
| TextDecoderStream                | encoding.TextDecoderStream               | false      | true         | true      |
| TextEncoderStream                | encoding.TextEncoderStream               | false      | true         | true      |
| TransformStream                  | streams.TransformStream                  | false      | true         | true      |
| URL                              | url.URL                                  | false      | true         | true      |
| URLPattern                       | urlPattern.URLPattern                    | false      | true         | true      |
| URLSearchParams                  | url.URLSearchParams                      | false      | true         | true      |
| MessageChannel                   | messagePort.MessageChannel               | false      | true         | true      |
| MessagePort                      | messagePort.MessagePort                  | false      | true         | true      |
| WritableStream                   | streams.WritableStream                   | false      | true         | true      |
| WritableStreamDefaultWriter      | streams.WritableStreamDefaultWriter      |            |              |           |
| WritableStreamDefaultController  | streams.WritableStreamDefaultController  |            |              |           |
| ReadableByteStreamController     | streams.ReadableByteStreamController     |            |              |           |
| ReadableStreamBYOBReader         | streams.ReadableStreamBYOBReader         |            |              |           |
| ReadableStreamBYOBRequest        | streams.ReadableStreamBYOBRequest        |            |              |           |
| ReadableStreamDefaultController  | streams.ReadableStreamDefaultController  |            |              |           |
| TransformStreamDefaultController | streams.TransformStreamDefaultController |            |              |           |
| ImageData                        | imageData.ImageData                      | false      | true         | true      |
| atob                             | base64.atob                              | true       | true         | true      |
| btoa                             | base64.btoa                              | true       | true         | true      |
| clearInterval                    | timers.clearInterval                     | true       | true         | true      |
| clearTimeout                     | timers.clearTimeout                      | true       | true         | true      |
| console                          | new console.Console(printer)             | false      | true         | true      |
| performance                      | performance.performance                  | true       | true         | true      |
| reportError                      | event.reportError                        | true       | true         | true      |
| setInterval                      | timers.setInterval                       | true       | true         | true      |
| setTimeout                       | timers.setTimeout                        | true       | true         | true      |
| structuredClone                  | messagePort.structuredClone              | true       | true         | true      |

Then from rust, provide:
`deno_web::deno_web::init(Arc<dyn BlobStoreTrait>, Option<Url>, bool, InMemoryBroadcastChannel)`
in the `extensions` field of your `RuntimeOptions`

Where:

- `Arc<dyn BlobStoreTrait>` can be provided by `BlobStore::default_arc()`
- `Option<Url>` provides an optional base URL for certain ops
- `bool` indicates whether window features are enabled at initialization
- `InMemoryBroadcastChannel` can be provided by `Default::default()`

## Dependencies

- **deno_webidl**: Provided by the `deno_webidl` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

- op_base64_decode
- op_base64_decode_into
- op_base64_encode
- op_base64_encode_from_buffer
- op_base64_atob
- op_base64_btoa
- op_encoding_normalize_label
- op_encoding_decode_single
- op_encoding_decode_utf8
- op_encoding_new_decoder
- op_encoding_decode
- op_encoding_encode_into
- op_encoding_encode_into_fallback
- op_blob_create_part
- op_blob_slice_part
- op_blob_read_part
- op_blob_remove_part
- op_blob_clone_part
- op_blob_create_object_url
- op_blob_revoke_object_url
- op_blob_from_object_url
- op_message_port_create_entangled
- op_message_port_post_message
- op_message_port_post_message_raw
- op_message_port_recv_message
- op_message_port_recv_message_sync
- op_compression_new
- op_compression_write
- op_compression_finish
- op_now
- op_time_origin
- op_defer
- op_geometry_get_enable_css_parser_features
- op_geometry_matrix_set_matrix_value
- op_geometry_matrix_to_string
- op_readable_stream_resource_allocate
- op_readable_stream_resource_allocate_sized
- op_readable_stream_resource_get_sink
- op_readable_stream_resource_write_error
- op_readable_stream_resource_write_buf
- op_readable_stream_resource_write_sync
- op_readable_stream_resource_close
- op_readable_stream_resource_await_close
- op_url_reparse
- op_url_parse
- op_url_get_serialization
- op_url_parse_with_base
- op_url_parse_search_params
- op_url_stringify_search_params
- op_urlpattern_parse
- op_urlpattern_process_match_input
- op_preview_entries
- op_broadcast_subscribe
- op_broadcast_unsubscribe
- op_broadcast_serialize
- op_broadcast_deserialize
- op_broadcast_free
- op_broadcast_send
- op_broadcast_recv
- DOMPointReadOnly
- DOMPoint
- DOMRectReadOnly
- DOMRect
- DOMQuad
- DOMMatrixReadOnly
- DOMMatrix
- ImageData
