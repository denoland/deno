// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { createHash } from "./mod.ts";

const millionAs = "a".repeat(1000000);

const testSetHex = [
  [
    "",
    "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26",
  ],
  [
    "abc",
    "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0",
  ],
  [
    "deno",
    "9e248199d744a8d810e7fda8207f98f27453bd6cb5a02965b5477d3d07516bbac6831009eedddadc8901d742dbfe3fd4afa770230a84e4d51bf30a0c99efa03c",
  ],
  [
    "The quick brown fox jumps over the lazy dog",
    "01dedd5de4ef14642445ba5f5b97c15e47b9ad931326e4b0727cd94cefc44fff23f07bf543139939b49128caf436dc1bdee54fcb24023a08d9403f9b4bf0d450",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "302d75b7947aa354a54872df954dc0dfe673cf60faedebdea7e9b22263a3bdf39e346a4f2868639836955396f186a67b02ec8e3365bdf59867070f81849c2c35",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "2141e94c719955872c455c83eb83e7618a9b523a0ee9f118e794fbff8b148545c8e8caabef08d8cfdb1dfb36b4dd81cc48bfc77e7f85632197b882fd9c4384e0",
  ],
  [
    millionAs,
    "3c3a876da14034ab60627c077bb98f7e120a2a5370212dffb3385a18d4f38859ed311d0a9d5141ce9cc5c66ee689b266a8aa18ace8282a0e0db596c90b0a7b87",
  ],
];

const testSetBase64 = [
  [
    "",
    "pp9zzKI6msXItWfcGFp1bpfJghZP4lhZ4NHcwUdcgKYVshI68fX5TBHj6UAsOsVY9QAZnZW20+MBdYWGKB3NJg==",
  ],
  [
    "abc",
    "t1GFCxpXFopWk82SS2sJbgj2IYJ0RPcNiE9dAkDScS4Q4RbpGSrzyRp+xXZH45NAVzQLTPQI1aVlkvgnTuxT8A==",
  ],
  [
    "deno",
    "niSBmddEqNgQ5/2oIH+Y8nRTvWy1oClltUd9PQdRa7rGgxAJ7t3a3IkB10Lb/j/Ur6dwIwqE5NUb8woMme+gPA==",
  ],
  [
    "The quick brown fox jumps over the lazy dog",
    "Ad7dXeTvFGQkRbpfW5fBXke5rZMTJuSwcnzZTO/ET/8j8Hv1QxOZObSRKMr0Ntwb3uVPyyQCOgjZQD+bS/DUUA==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "MC11t5R6o1SlSHLflU3A3+Zzz2D67evep+myImOjvfOeNGpPKGhjmDaVU5bxhqZ7AuyOM2W99ZhnBw+BhJwsNQ==",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "IUHpTHGZVYcsRVyD64PnYYqbUjoO6fEY55T7/4sUhUXI6Mqr7wjYz9sd+za03YHMSL/Hfn+FYyGXuIL9nEOE4A==",
  ],
  [
    millionAs,
    "PDqHbaFANKtgYnwHe7mPfhIKKlNwIS3/szhaGNTziFntMR0KnVFBzpzFxm7mibJmqKoYrOgoKg4NtZbJCwp7hw==",
  ],
];

test("[hash/sha3-512] testSha3-512Hex", () => {
  for (const [input, output] of testSetHex) {
    const sha3 = createHash("sha3-512");
    assertEquals(sha3.update(input).toString(), output);
    sha3.dispose();
  }
});

test("[hash/sha3-512] testSha3-512Base64", () => {
  for (const [input, output] of testSetBase64) {
    const sha3 = createHash("sha3-512");
    assertEquals(sha3.update(input).toString("base64"), output);
    sha3.dispose();
  }
});
