import { assertEquals } from "./test_deps.ts";
import {
  convertBase64ToBase64url,
  convertBase64urlToBase64,
} from "../base64/base64url.ts";
import { convertUint8ArrayToBase64 } from "../base64/base64.ts";

const oneBase64 = "c3ViamVjdHM/X2Q9MQ==";
const oneBase64url = "c3ViamVjdHM_X2Q9MQ";
const twoBase64 = "SGVsbG8gV29ybGQ=";
const twoBase64url = "SGVsbG8gV29ybGQ";

Deno.test("convertBase64ToBase64urlTest", function (): void {
  assertEquals(convertBase64ToBase64url(oneBase64), oneBase64url);
  assertEquals(convertBase64ToBase64url(twoBase64), twoBase64url);
});

Deno.test("convertBase64urlToBase64Test", function (): void {
  assertEquals(convertBase64urlToBase64(oneBase64url), oneBase64);
  assertEquals(convertBase64urlToBase64(twoBase64url), twoBase64);
});

Deno.test("convertStringToBase64urlTest", function (): void {
  assertEquals(
    convertBase64ToBase64url(
      convertUint8ArrayToBase64(new TextEncoder().encode(">?>d?ß")),
    ),
    "Pj8-ZD_Dnw",
  );
});
