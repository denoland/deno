// Copyright 2018-2025 the Deno authors. MIT license.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import crypto from "node:crypto";
import console from "node:console";

const keyTypes = [
  {
    type: "rsa",
    modulusLength: 2048,
  },
  {
    type: "rsa",
    modulusLength: 3072,
  },
  {
    type: "rsa-pss",
    modulusLength: 2048,
  },
  {
    type: "rsa-pss",
    modulusLength: 3072,
  },
  {
    type: "rsa-pss",
    modulusLength: 2048,
    saltLength: 32,
  },
  {
    type: "rsa-pss",
    modulusLength: 2048,
    hashAlgorithm: "sha512",
  },
  {
    type: "dsa",
    modulusLength: 2048,
  },
  {
    type: "dsa",
    modulusLength: 3072,
  },
  {
    type: "ec",
    namedCurve: "P-224",
  },
  {
    type: "ec",
    namedCurve: "P-256",
  },
  {
    type: "ec",
    namedCurve: "P-384",
  },
  {
    type: "x25519",
  },
  {
    type: "ed25519",
  },
  {
    type: "dh",
    group: "modp14",
  },
];

const data = "Hello, World!";

const entries = [];

for (const keyType of keyTypes) {
  console.log(keyType);
  const { privateKey, publicKey } = crypto.generateKeyPairSync(keyType.type, {
    modulusLength: keyType.modulusLength,
    namedCurve: keyType.namedCurve,
    group: keyType.group,
    saltLength: keyType.saltLength,
    hashAlgorithm: keyType.hashAlgorithm,
  });

  let name = keyType.type;
  if (keyType.type === "rsa-pss") {
    name += `_${keyType.modulusLength}_${keyType.saltLength ?? "nosalt"}_${
      keyType.hashAlgorithm ?? "nohash"
    }`;
  } else if (keyType.type === "rsa" || keyType.type === "dsa") {
    name += `_${keyType.modulusLength}`;
  } else if (keyType.type === "ec") {
    name += `_${keyType.namedCurve}`;
  } else if (keyType.type === "dh") {
    name += `_${keyType.group}`;
  }

  exportAndWrite(name, privateKey, "pem", "pkcs8");
  exportAndWrite(name, privateKey, "der", "pkcs8");
  exportAndWrite(name, publicKey, "pem", "spki");
  exportAndWrite(name, publicKey, "der", "spki");

  if (keyType.type === "rsa") {
    exportAndWrite(name, privateKey, "pem", "pkcs1");
    exportAndWrite(name, privateKey, "der", "pkcs1");
    exportAndWrite(name, publicKey, "pem", "pkcs1");
    exportAndWrite(name, publicKey, "der", "pkcs1");
  }
  if (keyType.type === "ec") {
    exportAndWrite(name, privateKey, "pem", "sec1");
    exportAndWrite(name, privateKey, "der", "sec1");
  }

  let signed;
  if (keyType.type === "ed25519") {
    signed = crypto
      .sign(null, Buffer.from(data), privateKey)
      .toString("base64");
  } else if (keyType.type !== "x25519" && keyType.type !== "dh") {
    console.log("signing", keyType.type);
    signed = crypto
      .createSign("sha512")
      .update(data)
      .sign(privateKey, "base64");
  }

  entries.push({
    name,
    keyType: keyType.type,
    signed,
  });
}

writeFileSync(
  join("tests", "unit_node", "crypto", "testdata", "asymmetric.json"),
  JSON.stringify(entries, null, 2),
);

function exportAndWrite(name, key, format, type) {
  const pem = key.export({
    format,
    type,
  });
  const filename = join(
    "tests",
    "unit_node",
    "crypto",
    "testdata",
    "asymmetric",
    `${name}.${type}.${format}`,
  );
  writeFileSync(filename, pem);
}
