import "http://127.0.0.1:4545/run/import_compression/gziped";
import "http://127.0.0.1:4545/run/import_compression/brotli";

console.log(
  await fetch(
    "http://127.0.0.1:4545/run/import_compression/gziped",
  ).then((res) => res.text()),
);
console.log(
  await fetch(
    "http://127.0.0.1:4545/run/import_compression/brotli",
  ).then((res) => res.text()),
);
