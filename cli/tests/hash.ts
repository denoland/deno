/* eslint-disable @typescript-eslint/no-explicit-any */

const { args } = Deno;
import { createHash, SupportedAlgorithm } from "../../std/hash/mod.ts";
import { Md5 } from "../../std/hash/md5.ts";
import { SHA1 } from "../../std/hash/sha1.ts";
import { SHA256 } from "../../std/hash/sha256.ts";
import { SHA512 } from "../../std/hash/sha512.ts";
import { SHA3_225, SHA3_256, SHA3_384, SHA3_512 } from "../../std/hash/sha3.ts";

if (args.length < 3) Deno.exit(0);

const method = args[0];
const alg = args[1];
const inputFile = args[2];

function getJsHash(alg: string): any {
  switch (alg) {
    case "md5":
      return new Md5();
    case "sha1":
      return new SHA1();
    case "sha224":
      return new SHA256(true);
    case "sha256":
      return new SHA256();
    case "sha3-224":
      return new SHA3_225();
    case "sha3-256":
      return new SHA3_256();
    case "sha3-384":
      return new SHA3_384();
    case "sha3-512":
      return new SHA3_512();
    case "sha512":
      return new SHA512();
    default:
      return null;
  }
}

const f = Deno.openSync(inputFile, { read: true });
const buffer = Deno.readAllSync(f);
f.close();

let hash = null;

console.time("hash");
if (method === "rust") {
  hash = createHash(alg as SupportedAlgorithm);
} else if (method === "js") {
  hash = getJsHash(alg);
}

if (hash === null) {
  console.log(`unknown hash: ${alg}`);
  Deno.exit(1);
}

hash.update(buffer);
hash.digest();
console.timeEnd("hash");
