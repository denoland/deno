import { loadTestLibrary } from "../../../test_napi/common.js";

const lib = loadTestLibrary();

Deno.bench("warmup", () => {});
Deno.bench("napi_get_undefined", () => lib.test_get_undefined(0));
