// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { crypto, type DigestAlgorithm } from "./mod.ts";
import { toHashString } from "./to_hash_string.ts";
import { repeat } from "../bytes/repeat.ts";
import { assertEquals } from "../assert/mod.ts";

const encoder = new TextEncoder();

const millionAs = "a".repeat(1000000);

// Simple periodic data, but the periods shouldn't line up with any block sizes.
const aboutAMeg = repeat(
  new Uint8Array(1237).fill(0).map((_, i) => i % 251),
  839,
);

// These should all be equivalent.
const slicedView = new Int16Array(aboutAMeg.buffer, 226, 494443);
const slicedCopy = new Uint8Array(aboutAMeg.slice(226, 226 + 16 / 8 * 494443));
const bufferCopy = slicedCopy.buffer;

const testSetHex: Record<string, [string | BufferSource, string][]> = {
  MD5: [
    ["", "d41d8cd98f00b204e9800998ecf8427e"],
    ["abc", "900150983cd24fb0d6963f7d28e17f72"],
    ["deno", "c8772b401bc911da102a5291cc4ec83b"],
    [
      "The quick brown fox jumps over the lazy dog",
      "9e107d9d372bb6826bd81d3542a419d6",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "3b0c8ac703f828b04c6c197006d17218",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "014842d480b571495a4a0363793f7367",
    ],
    [millionAs, "7707d6ae4e027c70eea2a935c2296f21"],
    [aboutAMeg, "65ee3c415a2316553ebf2fdb2ccafd0b"],
    [slicedView, "81f7e24f254ca2af692188d17b5103d8"],
    [slicedCopy, "81f7e24f254ca2af692188d17b5103d8"],
    [bufferCopy, "81f7e24f254ca2af692188d17b5103d8"],
  ],
  "SHA-1": [
    ["", "da39a3ee5e6b4b0d3255bfef95601890afd80709"],
    ["abc", "a9993e364706816aba3e25717850c26c9cd0d89d"],
    ["deno", "bb3d8e712d9e7ad4af08d4a38f3f52d9683d58eb"],
    [
      "The quick brown fox jumps over the lazy dog",
      "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "c2db330f6083854c99d4b5bfb6e8f29f201be699",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "0098ba824b5c16427bd7a1122a5a442a25ec644d",
    ],
    [millionAs, "34aa973cd4c4daa4f61eeb2bdbad27316534016f"],
    [aboutAMeg, "74de0faec24034e7415e7a6ee379e509b29985b2"],
    [slicedView, "b0161602fcdd324d2d0222b5c8d2873ff1f6452e"],
    [slicedCopy, "b0161602fcdd324d2d0222b5c8d2873ff1f6452e"],
    [bufferCopy, "b0161602fcdd324d2d0222b5c8d2873ff1f6452e"],
  ],
  "SHA-256": [
    ["", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
    ["abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"],
    [
      "deno",
      "e872e7bd2ae6abcf13a4c834029a342c882c1162ebf77b6720968b2000312ffb",
    ],
    [
      "The quick brown fox jumps over the lazy dog",
      "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
    ],
    [
      millionAs,
      "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
    ],
    [
      aboutAMeg,
      "ce0ae911a08c37d8e25605bc209c13e870ab3c4a40a7610ea3af989d9b0a00dd",
    ],
    [
      slicedView,
      "38fa97da941ae64bc1ec0d28fa14023e8041fd31857053d387d97e0ea1498203",
    ],
    [
      slicedCopy,
      "38fa97da941ae64bc1ec0d28fa14023e8041fd31857053d387d97e0ea1498203",
    ],
    [
      bufferCopy,
      "38fa97da941ae64bc1ec0d28fa14023e8041fd31857053d387d97e0ea1498203",
    ],
  ],
  "SHA-512": [
    [
      "",
      "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
    ],
    [
      "abc",
      "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
    ],
    [
      "deno",
      "05b6ef7b13673c57c455d968cb7acbdd4fe10e24f25520763d69025d768d14124987b14e3aff8ff1565aaeba1c405fc89cc435938ff46a426f697b0f509e3799",
    ],
    [
      "The quick brown fox jumps over the lazy dog",
      "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "962b64aae357d2a4fee3ded8b539bdc9d325081822b0bfc55583133aab44f18bafe11d72a7ae16c79ce2ba620ae2242d5144809161945f1367f41b3972e26e04",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "01d35c10c6c38c2dcf48f7eebb3235fb5ad74a65ec4cd016e2354c637a8fb49b695ef3c1d6f7ae4cd74d78cc9c9bcac9d4f23a73019998a7f73038a5c9b2dbde",
    ],
    [
      millionAs,
      "e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973ebde0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b",
    ],
    [
      aboutAMeg,
      "b3d3a7531e6bea36639bd9cf5a5c462f32d4f74a4b9878aad7405149d7962ad02e4cc1922133c43e9a2685f2927345a72c697144cbd69a895778126c1c59d455",
    ],
    [
      slicedView,
      "b7e29c5e61c67f5332740e01a1932be71aee0baf8e6d3156027585948cd58abbcf302de41978b0de26a0fb768708351963c6c01c1198e0dae7deaee448632445",
    ],
    [
      slicedCopy,
      "b7e29c5e61c67f5332740e01a1932be71aee0baf8e6d3156027585948cd58abbcf302de41978b0de26a0fb768708351963c6c01c1198e0dae7deaee448632445",
    ],
    [
      bufferCopy,
      "b7e29c5e61c67f5332740e01a1932be71aee0baf8e6d3156027585948cd58abbcf302de41978b0de26a0fb768708351963c6c01c1198e0dae7deaee448632445",
    ],
  ],
  "SHA3-256": [
    ["", "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"],
    ["abc", "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"],
    [
      "deno",
      "74a6286af90f8775d74080f864cf80b11eecf6f14d325c5ef8c9f7ccc8055517",
    ],
    [
      "The quick brown fox jumps over the lazy dog",
      "69070dda01975c8c120c3aada1b282394e7f032fa9cf32f4cb2259a0897dfc04",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "f6fe8de5c8f5014786f07e9f7b08130f920dd55e587d47021686b26cf2323deb",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "043d104b5480439c7acff8831ee195183928d9b7f8fcb0c655a086a87923ffee",
    ],
    [
      millionAs,
      "5c8875ae474a3634ba4fd55ec85bffd661f32aca75c6d699d0cdcb6c115891c1",
    ],
    [
      aboutAMeg,
      "ff7934eb30afb91390adbd02ef2bf808eeac30bb4a7779f346a71962610874bd",
    ],
    [
      slicedView,
      "ec3e5fb22a6a7e2f404cb10fca361a3edc3a6f7eaaeb83a4142adf3f89e5b1d5",
    ],
    [
      slicedCopy,
      "ec3e5fb22a6a7e2f404cb10fca361a3edc3a6f7eaaeb83a4142adf3f89e5b1d5",
    ],
    [
      bufferCopy,
      "ec3e5fb22a6a7e2f404cb10fca361a3edc3a6f7eaaeb83a4142adf3f89e5b1d5",
    ],
  ],
  "SHA3-512": [
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
    [
      aboutAMeg,
      "61bbdae5203bbf8a9effd083da83ebf18951668e658a810987ea2feb1fb810be5800fb03489a99e9f25979aa6c345477036afabcda612066b3c1213a72c05534",
    ],
    [
      slicedView,
      "8b43aec6757a768580ed9bb74e373040a25692054d5097cf0ab8f9b565c266ab6964aa02b1d54388b10bc80461f83dbc8cf9e59c8321124315b8058b1a057b2a",
    ],
    [
      slicedCopy,
      "8b43aec6757a768580ed9bb74e373040a25692054d5097cf0ab8f9b565c266ab6964aa02b1d54388b10bc80461f83dbc8cf9e59c8321124315b8058b1a057b2a",
    ],
    [
      bufferCopy,
      "8b43aec6757a768580ed9bb74e373040a25692054d5097cf0ab8f9b565c266ab6964aa02b1d54388b10bc80461f83dbc8cf9e59c8321124315b8058b1a057b2a",
    ],
  ],
  BLAKE3: [
    ["", "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"],
    ["abc", "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"],
    [
      "deno",
      "e5dd810dd67713fab4438e17516c7ea13a35666900ece70a561184ff68de8d79",
    ],
    [
      "The quick brown fox jumps over the lazy dog",
      "2f1514181aadccd913abd94cfa592701a5686ab23f8df1dff1b74710febc6d4a",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "86dd7cd514f2b1f6aaa34688ead22746f453e9d9ddeeca1ef124477507aefc9f",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "472c51290d607f100d2036fdcedd7590bba245e9adeb21364a063b7bb4ca81c7",
    ],
    [
      millionAs,
      "616f575a1b58d4c9797d4217b9730ae5e6eb319d76edef6549b46f4efe31ff8b",
    ],
    [
      aboutAMeg,
      "7fc79f34e187d62c474af7d57531a77f193ab6f2fae71c6de155b341cb592fe5",
    ],
    [
      slicedView,
      "8549694280dea254adb1b856779d2d4f09256004e7536bbf544a1859e66b5f9c",
    ],
    [
      slicedCopy,
      "8549694280dea254adb1b856779d2d4f09256004e7536bbf544a1859e66b5f9c",
    ],
    [
      bufferCopy,
      "8549694280dea254adb1b856779d2d4f09256004e7536bbf544a1859e66b5f9c",
    ],
  ],
  TIGER: [
    ["", "3293ac630c13f0245f92bbb1766e16167a4e58492dde73f3"],
    ["a", "77befbef2e7ef8ab2ec8f93bf587a7fc613e247f5f247809"],
    ["abc", "2aab1484e8c158f2bfb8c5ff41b57a525129131c957b5f93"],
    [
      "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
      "0f7bf9a19b9c58f2b7610df7e84f0ac3a71c631e7b53f78e",
    ],
    [millionAs, "6db0e2729cbead93d715c6a7d36302e9b3cee0d2bc314b41"],
    [aboutAMeg, "111764e3c4f512abce83c7ebdf061caca4f9a04177046509"],
    [slicedView, "affa436814964b03d0ab7d5743fcfdcaee2ad5ecb792e1eb"],
    [slicedCopy, "affa436814964b03d0ab7d5743fcfdcaee2ad5ecb792e1eb"],
    [bufferCopy, "affa436814964b03d0ab7d5743fcfdcaee2ad5ecb792e1eb"],
  ],
};

const testSetBase64: Record<string, string[][]> = {
  MD5: [
    ["", "1B2M2Y8AsgTpgAmY7PhCfg=="],
    ["abc", "kAFQmDzST7DWlj99KOF/cg=="],
    ["deno", "yHcrQBvJEdoQKlKRzE7IOw=="],
    ["The quick brown fox jumps over the lazy dog", "nhB9nTcrtoJr2B01QqQZ1g=="],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "OwyKxwP4KLBMbBlwBtFyGA==",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "AUhC1IC1cUlaSgNjeT9zZw==",
    ],
    [millionAs, "dwfWrk4CfHDuoqk1wilvIQ=="],
  ],
  "SHA-1": [
    ["", "2jmj7l5rSw0yVb/vlWAYkK/YBwk="],
    ["abc", "qZk+NkcGgWq6PiVxeFDCbJzQ2J0="],
    ["deno", "uz2OcS2eetSvCNSjjz9S2Wg9WOs="],
    [
      "The quick brown fox jumps over the lazy dog",
      "L9ThxnotKPzthJ7hu3bnORuT6xI=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "wtszD2CDhUyZ1LW/tujynyAb5pk=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "AJi6gktcFkJ716ESKlpEKiXsZE0=",
    ],
    [millionAs, "NKqXPNTE2qT2Husr260nMWU0AW8="],
  ],
  "SHA-256": [
    ["", "47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU="],
    ["abc", "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="],
    ["deno", "6HLnvSrmq88TpMg0Apo0LIgsEWLr93tnIJaLIAAxL/s="],
    [
      "The quick brown fox jumps over the lazy dog",
      "16j7swfXgJRpypq8sAguT41WUeRtPNt2LQLQvzfJ5ZI=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "s1Q5pKxvCUi21vnjxq8PX1kM4g8b3nCQ73lwaG7Gc4o=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "/+BU/nrgy23GXDr5th1SCfQ5hR20PQulmXM33xVGaOs=",
    ],
    [millionAs, "zcduXJkU+5KBocfihNc+Z/GAmkiklyAOBG05zMcRLNA="],
  ],
  "SHA-512": [
    [
      "",
      "z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==",
    ],
    [
      "abc",
      "3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw==",
    ],
    [
      "deno",
      "BbbvexNnPFfEVdloy3rL3U/hDiTyVSB2PWkCXXaNFBJJh7FOOv+P8VZarrocQF/InMQ1k4/0akJvaXsPUJ43mQ==",
    ],
    [
      "The quick brown fox jumps over the lazy dog",
      "B+VH2VhvanP3P7rAQ17XaVEhj7fQyNeIownXhUNru2Quk6JSqVTyORJUfR6KO17W4b/XCXghIz+gU489uFT+5g==",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "litkquNX0qT+497YtTm9ydMlCBgisL/FVYMTOqtE8Yuv4R1yp64Wx5ziumIK4iQtUUSAkWGUXxNn9Bs5cuJuBA==",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "AdNcEMbDjC3PSPfuuzI1+1rXSmXsTNAW4jVMY3qPtJtpXvPB1veuTNdNeMycm8rJ1PI6cwGZmKf3MDilybLb3g==",
    ],
    [
      millionAs,
      "5xhIPQznaWROLkLHvBW0Y44fmLE7IEQoVjKoA6+pc+veD/JEh36mCkywQyzld8Mb6wCcXCxJqi5OrbIXrYzAmw==",
    ],
  ],
  "SHA3-256": [
    ["", "p//G+L8e12ZRwUdWoGHWYvWA/03kO0n6gtgKS4D4Q0o="],
    ["abc", "Ophdp0/iJbIEXBcta9OQvYVfCG4+nVJbRr/iRRFDFTI="],
    ["deno", "dKYoavkPh3XXQID4ZM+AsR7s9vFNMlxe+Mn3zMgFVRc="],
    [
      "The quick brown fox jumps over the lazy dog",
      "aQcN2gGXXIwSDDqtobKCOU5/Ay+pzzL0yyJZoIl9/AQ=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "9v6N5cj1AUeG8H6fewgTD5IN1V5YfUcCFoaybPIyPes=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "BD0QS1SAQ5x6z/iDHuGVGDko2bf4/LDGVaCGqHkj/+4=",
    ],
    [millionAs, "XIh1rkdKNjS6T9VeyFv/1mHzKsp1xtaZ0M3LbBFYkcE="],
  ],
  "SHA3-512": [
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
  ],
  BLAKE3: [
    [
      "",
      "rxNJufX5oaagQE3qNtzJSZvLJcmtwRK3zJqTyuQfMmI=",
    ],
    [
      "abc",
      "ZDezrDhGUTP/tjt1JzqNtUjFWEZdedsD/TWcbNW9nYU=",
    ],
    [
      "deno",
      "5d2BDdZ3E/q0Q44XUWx+oTo1ZmkA7OcKVhGE/2jejXk=",
    ],
    [
      "The quick brown fox jumps over the lazy dog",
      "LxUUGBqtzNkTq9lM+lknAaVoarI/jfHf8bdHEP68bUo=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "ht181RTysfaqo0aI6tInRvRT6dnd7soe8SRHdQeu/J8=",
    ],
    [
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "RyxRKQ1gfxANIDb9zt11kLuiRemt6yE2SgY7e7TKgcc=",
    ],
    [
      millionAs,
      "YW9XWhtY1Ml5fUIXuXMK5ebrMZ127e9lSbRvTv4x/4s=",
    ],
  ],
};

Deno.test("[crypto/util/hex] testAllHex", async () => {
  for (const algorithm in testSetHex) {
    for (const [input, output] of testSetHex[algorithm]) {
      const data = typeof input === "string" ? encoder.encode(input) : input;
      const hash = await crypto.subtle.digest(
        algorithm as DigestAlgorithm,
        data,
      );
      assertEquals(toHashString(hash), output);
    }
  }
});

Deno.test("[crypto/util/base64] testAllBase64", async () => {
  for (const algorithm in testSetBase64) {
    for (const [input, output] of testSetBase64[algorithm]) {
      const data = typeof input === "string" ? encoder.encode(input) : input;
      const hash = await crypto.subtle.digest(
        algorithm as DigestAlgorithm,
        data,
      );
      assertEquals(toHashString(hash, "base64"), output);
    }
  }
});
