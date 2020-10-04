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
import { convertHexToBase64url, isExpired, setExpiration } from "./_util.ts";

Deno.test("[jwt] conversion", function (): void {
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

Deno.test("[jwt] isExpired", function (): void {
  // A specific date:
  const t1 = setExpiration(new Date("2020-01-01"));
  const t2 = setExpiration(new Date("2099-01-01"));
  // Ten seconds from now:
  const t3 = setExpiration(10);
  // One hour from now:
  const t4 = setExpiration(60 * 60);
  //  1 second from now:
  const t5 = setExpiration(1);
  //  1 second earlier:
  const t6 = setExpiration(-1);
  assertEquals(isExpired(t1), true);
  assertEquals(isExpired(t2), false);
  assertEquals(10, t3 - Math.round(Date.now() / 1000));
  assertEquals(isExpired(t4), false);
  assertEquals(isExpired(t5), false);
  assertEquals(isExpired(t6), true);
  // add leeway:
  assertEquals(isExpired(t6, 1500), false);
  assertEquals(setExpiration(10), setExpiration(new Date(Date.now() + 10000)));
});

Deno.test("[jwt] setExpiration", function (): void {
  assertEquals(setExpiration(10), setExpiration(new Date(Date.now() + 10000)));
});
