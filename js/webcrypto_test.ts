import { test, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

const encoder = new TextEncoder;
function bytesToHex(bytes: Uint8Array|ArrayBuffer) {
    let hex = "";
    for (let i = 0; i< bytes.byteLength; i++) {
        let h = (bytes[i] & 0xff).toString(16);
        if (h.length === 1) h = "0"+h;
        hex += h
    }
    return hex;
}
test(async function testSha1() {
    const bytes = encoder.encode("abcde")
    const res = await crypto.subtle.digest("SHA-1", bytes);
    let hash = bytesToHex(res)
    assertEqual(hash, "03de6c570bfe24bfc328ccd7ca46b76eadaf4334")
})

test(async function testSha256() {    
    const bytes = encoder.encode("abcde");
    const res = await crypto.subtle.digest("SHA-256", bytes);
    const hash = bytesToHex(res);
    assertEqual(hash, "36bbe50ed96841d10443bcb670d6554f0a34b761be67ec9c4a8ad2c0c44ca42c")
})

test(async function testSha384() {    
    const bytes = encoder.encode("abcde");
    const res = await crypto.subtle.digest("SHA-384", bytes);
    const hash = bytesToHex(res);
    assertEqual(hash, "4c525cbeac729eaf4b4665815bc5db0c84fe6300068a727cf74e2813521565abc0ec57a37ee4d8be89d097c0d2ad52f0")
})

test(async function testSha512() {    
    const bytes = encoder.encode("abcde");
    const res = await crypto.subtle.digest("SHA-512", bytes);
    const hash = bytesToHex(res);
    assertEqual(hash, "878ae65a92e86cac011a570d4c30a7eaec442b85ce8eca0c2952b5e3cc0628c2e79d889ad4d5c7c626986d452dd86374b6ffaa7cd8b67665bef2289a5c70b0a1")
})

test(async function testUnsupportedHashFunc() {
    let err;
    try {
        await crypto.subtle.digest("SHA-52", new Uint8Array([0,1,2]));
    } catch (e) {
        err = e;
    }
    assert(err !== void 0);
})