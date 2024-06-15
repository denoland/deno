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
import * as infra from "ext:deno_web/00_infra.js";
import * as DOMException from "ext:deno_web/01_dom_exception.js";
import * as mimesniff from "ext:deno_web/01_mimesniff.js";
import * as event from "ext:deno_web/02_event.js";
import * as structuredClone from "ext:deno_web/02_structured_clone.js";
import * as timers from "ext:deno_web/02_timers.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as base64 from "ext:deno_web/05_base64.js";
import * as streams from "ext:deno_web/06_streams.js";
import * as encoding from "ext:deno_web/08_text_encoding.js";
import * as file from "ext:deno_web/09_file.js";
import * as fileReader from "ext:deno_web/10_filereader.js";
import * as location from "ext:deno_web/12_location.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import * as compression from "ext:deno_web/14_compression.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as imageData from "ext:deno_web/16_image_data.js";
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
| performance                      | performance.performance                  | true       | true         | true      |
| reportError                      | event.reportError                        | true       | true         | true      |
| setInterval                      | timers.setInterval                       | true       | true         | true      |
| setTimeout                       | timers.setTimeout                        | true       | true         | true      |
| structuredClone                  | messagePort.structuredClone              | true       | true         | true      |

Then from rust, provide:
`deno_web::deno_web::init_ops_and_esm::<Permissions>(Arc<BlobStore>, Option<Url>)`
in the `extensions` field of your `RuntimeOptions`

Where:

- `Permissions` is a struct implementing `deno_web::TimersPermission`
- `Arc<BlobStore>` can be provided by `Default::default()`
- `Option<Url>` provides an optional base URL for certain ops

## Dependencies

- **deno_webidl**: Provided by the `deno_webidl` crate
- **deno_console**: Provided by the `deno_console` crate
- **deno_url**: Provided by the `deno_url` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

- op_base64_decode
- op_base64_encode
- op_base64_atob
- op_base64_btoa
- op_encoding_normalize_label
- op_encoding_decode_single
- op_encoding_decode_utf8
- op_encoding_new_decoder
- op_encoding_decode
- op_encoding_encode_into
- op_blob_create_part
- op_blob_slice_part
- op_blob_read_part
- op_blob_remove_part
- op_blob_create_object_url
- op_blob_revoke_object_url
- op_blob_from_object_url
- op_message_port_create_entangled
- op_message_port_post_message
- op_message_port_recv_message
- op_message_port_recv_message_sync
- op_compression_new
- op_compression_write
- op_compression_finish
- op_now
- op_defer
- op_transfer_arraybuffer
- op_readable_stream_resource_allocate
- op_readable_stream_resource_allocate_sized
- op_readable_stream_resource_get_sink
- op_readable_stream_resource_write_error
- op_readable_stream_resource_write_buf
- op_readable_stream_resource_write_sync
- op_readable_stream_resource_close
- op_readable_stream_resource_await_close
