import "http://127.0.0.1:4545/cli/tests/053_import_compression/brotli";
import "http://127.0.0.1:4545/cli/tests/053_import_compression/gziped";

console.log(
  await fetch(
    "http://127.0.0.1:4545/cli/tests/053_import_compression/gziped",
  ).then((res) => res.text()),
);
console.log(
  await fetch(
    "http://127.0.0.1:4545/cli/tests/053_import_compression/brotli",
  ).then((res) => res.text()),
);
