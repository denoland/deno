import { assertEquals } from "./test_deps.ts";
import {
  convertBase64ToUint8Array,
  convertUint8ArrayToBase64,
  convertStringToBase64,
  convertBase64ToString,
} from "../base64/base64.ts";

const str1 = "Hello ☸☹☺☻☼☾☿ World ✓";
const str2 = "Man Ё𤭢";
const uint8Array1 = new TextEncoder().encode(str1);
const uint8Array2 = new TextEncoder().encode(str2);

Deno.test("convertBase64ToUint8ArrayAndBackTest", function (): void {
  assertEquals(
    uint8Array1,
    convertBase64ToUint8Array(convertUint8ArrayToBase64(uint8Array1)),
  );
  assertEquals(
    uint8Array2,
    convertBase64ToUint8Array(convertUint8ArrayToBase64(uint8Array2)),
  );
});

Deno.test("convertBase64ToStringAndBackTest", function (): void {
  assertEquals(str1, convertBase64ToString(convertStringToBase64(str1)));
  assertEquals(str2, convertBase64ToString(convertStringToBase64(str2)));
});

Deno.test("convertMultiByteCharactersTest", function (): void {
  assertEquals(
    convertBase64ToString(
      convertUint8ArrayToBase64(
        convertBase64ToUint8Array(convertStringToBase64(str1)),
      ),
    ),
    str1,
  );
  assertEquals(
    convertBase64ToString(
      convertUint8ArrayToBase64(
        convertBase64ToUint8Array(convertStringToBase64(str2)),
      ),
    ),
    str2,
  );
});
