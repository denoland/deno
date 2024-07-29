# deno_url

**This crate implements the URL, and URLPattern APIs for Deno.**

URL Spec: https://url.spec.whatwg.org/ URLPattern Spec:
https://wicg.github.io/urlpattern/

## Usage Example

From javascript, include the extension's source, and assign `URL`, `URLPattern`,
and `URLSearchParams` to the global scope:

```javascript
import * as url from "ext:deno_url/00_url.js";
import * as urlPattern from "ext:deno_url/01_urlpattern.js";

Object.defineProperty(globalThis, "URL", {
  value: url.URL,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "URLPattern", {
  value: url.URLPattern,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "URLSearchParams", {
  value: url.URLSearchParams,
  enumerable: false,
  configurable: true,
  writable: true,
});
```

Then from rust, provide `deno_url::deno_url::init_ops_and_esm()` in the
`extensions` field of your `RuntimeOptions`

## Dependencies

- **deno_webidl**: Provided by the `deno_webidl` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

- op_url_reparse
- op_url_parse
- op_url_get_serialization
- op_url_parse_with_base
- op_url_parse_search_params
- op_url_stringify_search_params
- op_urlpattern_parse
- op_urlpattern_process_match_input
