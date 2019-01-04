import { test, assertEqual } from "./test_util.ts";
import * as deno from "deno";

const encoder = new TextEncoder;
const decoder = new TextDecoder;
test(async function testSha1() {
    const bytes = encoder.encode("abcde")
    const res = await crypto.subtle.digest("SHA-1", bytes);
    const hash = decoder.decode(res);
    console.log(res,hash)
    assertEqual(hash, "03de6c570bfe24bfc328ccd7ca46b76eadaf4334")
})

test(async function testSha256() {    
    const bytes = encoder.encode("hello, world");
    const res = await crypto.subtle.digest("SHA-256", bytes);
    const hash = decoder.decode(res);
    assertEqual(hash, "09ca7e4eaa6e8ae9c7d261167129184883644d07dfba7cbfbc4c8a2e08360d5b")
})