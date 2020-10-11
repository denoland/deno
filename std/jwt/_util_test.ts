import {
  convertBase64ToBase64url,
  convertBase64urlToBase64,
} from "./base64/base64url.ts";
import {
  convertBase64ToUint8Array,
  convertUint8ArrayToBase64,
} from "./base64/base64.ts";
import {
  decodeString as convertHexToUint8Array,
  encodeToString as convertUint8ArrayToHex,
} from "../encoding/hex.ts";
import { assertEquals } from "../testing/asserts.ts";
import { convertHexToBase64url } from "./_util.ts";

Deno.test("[jwt] conversion", function () {
  const hex1 =
    "a4a99a8e21149ccbc5c5aabd310e5d5208b12db90dff749171d5014b688ce808";
  const hex2 = convertUint8ArrayToHex(
    convertBase64ToUint8Array(
      convertBase64urlToBase64(
        convertBase64ToBase64url(
          convertUint8ArrayToBase64(
            convertHexToUint8Array(
              convertUint8ArrayToHex(
                convertBase64ToUint8Array(
                  convertBase64urlToBase64(convertHexToBase64url(hex1)),
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  assertEquals(hex1, hex2);
});
