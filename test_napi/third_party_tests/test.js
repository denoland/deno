// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

const { strictEqual, deepEqual } = require("assert/strict");
const assert = require("assert");

Deno.test("dprint-node", () => {
  const dprint = require("dprint-node");

  const result = dprint.format(
    "hello.js",
    "function x(){let a=1;return a;}",
    {
      lineWidth: 100,
      semiColons: "asi",
    },
  );
  strictEqual(typeof result, "string");
  assert(result.length > 0);
});

// Deno.test("@napi-rs/canvas", () => {
//   const { createCanvas } = require('@napi-rs/canvas')

//   const canvas = createCanvas(300, 320)
//   const ctx = canvas.getContext('2d')

//   // Do some random stuff, make sure it works.
//   ctx.lineWidth = 10;
//   ctx.strokeStyle = '#03a9f4';
//   ctx.fillStyle = '#03a9f4';
//   ctx.strokeRect(75, 140, 150, 110);
//   ctx.fillRect(130, 190, 40, 60);
//   ctx.beginPath()
//   ctx.moveTo(50, 140)
//   ctx.lineTo(150, 60)
//   ctx.lineTo(250, 140)
//   ctx.closePath()
//   ctx.stroke()

//   canvas.encode('png');
// });

Deno.test("@parcel/hash", () => {
  const { hashString, hashBuffer, Hash } = require("@parcel/hash");
  strictEqual(hashString("Hello, Deno!"), "210a1f862b67f327");
  strictEqual(hashBuffer(Deno.core.encode("Hello, Deno!")), "210a1f862b67f327");

  const hasher = new Hash();
  hasher.writeString("Hello, Deno!");
  strictEqual(hasher.finish(), "210a1f862b67f327");
});

Deno.test("@tauri-apps/cli", () => {
  const _ = require("@tauri-apps/cli");
});

Deno.test("@parcel/css", async () => {
  const { transform } = require("@parcel/css");
  const result = transform({
    filename: "test.css",
    minify: false,
    targets: {
      safari: 4 << 16,
      firefox: 3 << 16 | 5 << 8,
      opera: 10 << 16 | 5 << 8,
    },
    code: Deno.core.encode(`
    @import "foo.css";
    @import "bar.css" print;
    @import "baz.css" supports(display: grid);
    .foo {
        composes: bar;
        composes: baz from "baz.css";
        color: pink;
    }
    .bar {
        color: red;
        background: url(test.jpg);
    }
    `),
    drafts: {
      nesting: true,
    },
    cssModules: true,
    analyzeDependencies: true,
  });
  assert(result.exports);
  assert(result.dependencies);
  assert(result.code instanceof Uint8Array);
});

Deno.test("@swc/core transform", () => {
  const swc = require("@swc/core");

  swc.transform("const x: number = 69;", {
    filename: "input.js",
    sourceMaps: true,
    isModule: false,
    jsc: {
      parser: {
        syntax: "typescript",
      },
      transform: {},
    },
  }).then((output) => {
    assert(typeof output.code == "string");
    assert(typeof output.map == "string");
  });
});

Deno.test("snappy", () => {
  const snappy = require("snappy");
  const original = Deno.readFileSync("test_parcel_optimizer.jpeg");
  const compressed = snappy.compressSync(original);
  assert(compressed instanceof Uint8Array);
});

Deno.test("@node-rs/bcrypt", () => {
  const bcrypt = require("@node-rs/bcrypt");
  const hash = bcrypt.hashSync("Hello, Deno!");
  const hash2 = bcrypt.hashSync(Deno.core.encode("Hello, Deno!"));

  assert(!bcrypt.compareSync("nani?", hash));
  assert(bcrypt.compareSync("Hello, Deno!", hash));
  assert(bcrypt.compareSync("Hello, Deno!", hash2));
});

Deno.test("@node-rs/argon2", () => {
  const { hash, verify } = require("@node-rs/argon2");
  const input = Deno.core.encode("Hello, Deno!");
  hash(input).then(async (hashed) => {
    assert(await verify(hashed, "Hello, Deno!"));
  });
});

Deno.test("@node-rs/xxhash", () => {
  const { xxh3 } = require("@node-rs/xxhash");
  strictEqual(xxh3.xxh64("Hello, Deno!"), 2380750014133039911n);
});

Deno.test("@node-rs/crc32", () => {
  const { crc32, crc32c } = require("@node-rs/crc32");
  const a = crc32("Hello, Deno!");
  const b = crc32c("Hello, Deno!");
  assert(typeof a == "number");
  assert(typeof b == "number");
});

Deno.test("@napi-rs/escape", () => {
  const { escapeHTML } = require("@napi-rs/escape");
  const escaped = escapeHTML(`<div>{props.getNumber()}</div>`);
  strictEqual(escaped, "&lt;div&gt;{props.getNumber()}&lt;&#x2f;div&gt;");
});

Deno.test("@napi-rs/uuid", () => {
  const { v4 } = require("@napi-rs/uuid");
  const uuid = v4();
  assert(typeof uuid == "string");
});

// Deno.test("ffi-napi", () => {
//   const ffi = require('ffi-napi');
//   const libm = ffi.Library('libm', {
//     'ceil': [ 'double', [ 'double' ] ]
//   });
//   libm.ceil(1.5); // 2
// })

Deno.test("@napi-rs/blake-hash", async () => {
  const { blake3, Blake3Hasher } = require("@napi-rs/blake-hash");
  // deno-fmt-ignore
  const hash = new Uint8Array([
    39, 227,  21,  85, 191, 248,  17, 199,
   213,  49, 181, 224, 192,  23, 144, 216,
     1,  87, 161, 124, 146, 124, 204, 208,
    28,  61, 194,  32, 157, 201, 223, 192
  ]);
  const hashed = blake3("Hello, Deno!");
  deepEqual(new Uint8Array(hashed), hash);

  const hasher = new Blake3Hasher();
  hasher.update("Hello, Deno!");
  const hex = hasher.digest("hex");
  strictEqual(
    hex,
    "27e31555bff811c7d531b5e0c01790d80157a17c927cccd01c3dc2209dc9dfc0",
  );
});

Deno.test("@napi-rs/lzma", () => {
  const lzma = require("@napi-rs/lzma/lzma");
  lzma.compress("Hello, Deno!").then(async (compressed) => {
    assert(compressed instanceof Uint8Array);
    const decompressed = await lzma.decompress(compressed);
    assert(decompressed instanceof Uint8Array);
    deepEqual(decompressed, Deno.core.encode("Hello, Deno!"));
  });
});

Deno.test("@napi-rs/ed25519", () => {
  const ed25519 = require("@napi-rs/ed25519");
  const message = Deno.core.encode("Hello, Deno!");
  const { publicKey, privateKey } = ed25519.generateKeyPair();
  const signature = ed25519.sign(privateKey, message);
  assert(ed25519.verify(publicKey, message, signature));
});

Deno.test("@parcel/optimizer-image", () => {
  const optimizer = require("@parcel/optimizer-image/native");
  const original = Deno.readFileSync("test_parcel_optimizer.jpeg");
  const optimized = optimizer.optimize("jpeg", original);
  assert(optimized instanceof Uint8Array);
  assert(optimized.byteLength < original.byteLength);
});

Deno.test("@parcel/transformer-js", () => {
  const { default: Transformer } = require("@parcel/transformer-js");
  const CONFIG = Symbol.for("parcel-plugin-config");
  assert(Transformer[CONFIG]);
});

Deno.test("@prisma/engines", async () => {
  const engine = require("@prisma/engines");
  const glob = require("glob");

  assert(engine.enginesVersion);
  const path = engine.getEnginesPath();

  strictEqual(engine.getCliQueryEngineBinaryType(), "libquery-engine");
  const file = glob.sync(path + "*.node");
  strictEqual(file.length, 1);
  const dylib = file[0];

  const { QueryEngine, version } = require(dylib);

  assert(version());
  const qEngine = new QueryEngine({
    datamodel: `
    generator client {
      provider = "prisma-client-js"
    }
    
    datasource db {
      provider = "sqlite"
      url      = "file:./prisma_client_test.db"
    }

    model User {
      id      String   @default(cuid()) @id
      name    String
    }
    `,
    env: {},
    logQueries: true,
    ignoreEnvVarErrors: false,
    logLevel: "debug",
    configDir: ".",
  }, console.info);

  assert(qEngine);
  await qEngine.connect({ enableRawQueries: true });
  await qEngine.disconnect();
});

Deno.test("@parcel/watcher", () => {
  // const watcher = require("@parcel/watcher");
  // watcher.subscribe(".", console.info, {}).then(() => {
  //   watcher.unsubscribe(".", console.info, {});
  // });
});

Deno.test("@parcel/fs-search", async () => {
  const { findFirstFile } = require("@parcel/fs-search");
  const file = findFirstFile(
    [
      "./test/example_non_existent.js",
      "./test.js",
      "./test/example_non_existent2.js",
    ],
  );
  assert(typeof file == "string");
  strictEqual(file, "./test.js");
});

Deno.test("@napi-rs/notify", async () => {
  const { watch } = require("@napi-rs/notify");
  const unwatch = watch(".", console.info);
  unwatch();
});

Deno.test("skia-canvas", () => {
  // TODO(@littledivy): require loader should handle `./dir/` as `./dir/index.node`.
  //
  // const skia = require("skia-canvas");
  // console.log(skia);
});

// TODO(@littledivy): Don't run this in the CI.
Deno.test("usb-enum", async () => {
  const usb = require("usb-enum");
  const devices = await usb.list();
  assert(devices instanceof Array);
});

// TODO(@littledivy): Don't run this in the CI.
Deno.test("node-usb", () => {
  // const usb = require("usb");
  // const devices = usb.getDeviceList();
  // assert(devices instanceof Array);
});

Deno.test("@tensorflow/tfjs-node", () => {
  const tf = require("@tensorflow/tfjs-node");
  const x = tf.tensor1d([1, 2, Math.sqrt(2), -1]);
  x.print();
});

Deno.test("msgpackr-extract", () => {
  const msgpackrExtract = require("msgpackr-extract");
  const result = msgpackrExtract.extractStrings(new Uint8Array([0]));
  strictEqual(result, undefined);
});

// Deno.test("sqlite3", async () => {
//   const { Database } = require("sqlite3");
//   const db = new Database(":memory:");
//   db.close();
// });
