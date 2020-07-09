// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/camelcase */
import { assertEquals, assertThrows } from "../testing/asserts.ts";
import {
  Keccak224,
  Keccak256,
  Keccak384,
  Keccak512,
  Sha3_224,
  Sha3_256,
  Sha3_384,
  Sha3_512,
  Shake128,
  Shake256,
} from "./sha3.ts";
import * as hex from "../encoding/hex.ts";

const millionAs = "a".repeat(1000000);

const testSetSha3_224 = [
  ["", "6b4e03423667dbb73b6e15454f0eb1abd4597f9a1b078e3f5b5a6bc7"],
  ["abc", "e642824c3f8cf24ad09234ee7d3c766fc9a3a5168d0c94ad73b46fdf"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "8a24108b154ada21c9fd5574494479ba5c7e7ab76ef264ead0fcce33",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "f9019111996dcf160e284e320fd6d8825cabcd41a5ffdc4c5e9d64b6",
  ],
  [millionAs, "d69335b93325192e516a912e6d19a15cb51c6ed5c15243e7a7fd653c"],
];

const testSetSha3_256 = [
  ["", "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"],
  ["abc", "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "41c0dba2a9d6240849100376a8235e2c82e1b9998a999e21db32dd97496d3376",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "3fc5559f14db8e453a0a3091edbd2bc25e11528d81c66fa570a4efdcc2695ee1",
  ],
  [
    millionAs,
    "5c8875ae474a3634ba4fd55ec85bffd661f32aca75c6d699d0cdcb6c115891c1",
  ],
];

const testSetSha3_384 = [
  [
    "",
    "0c63a75b845e4f7d01107d852e4c2485c51a50aaaa94fc61995e71bbee983a2ac3713831264adb47fb6bd1e058d5f004",
  ],
  [
    "abc",
    "ec01498288516fc926459f58e2c6ad8df9b473cb0fc08c2596da7cf0e49be4b298d88cea927ac7f539f1edf228376d25",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "3a4f3b6284e571238884e95655e8c8a60e068e4059a9734abc08823a900d161592860243f00619ae699a29092ed91a16",
  ],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "991c665755eb3a4b6bbdfb75c78a492e8c56a22c5c4d7e429bfdbc32b9d4ad5aa04a1f076e62fea19eef51acd0657c22",
  ],
  [
    millionAs,
    "eee9e24d78c1855337983451df97c8ad9eedf256c6334f8e948d252d5e0e76847aa0774ddb90a842190d2c558b4b8340",
  ],
];

const testSetSha3_512 = [
  [
    "",
    "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26",
  ],
  [
    "abc",
    "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0",
  ],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "04a371e84ecfb5b8b77cb48610fca8182dd457ce6f326a0fd3d7ec2f1e91636dee691fbe0c985302ba1b0d8dc78c086346b533b49c030d99a27daf1139d6e75e",
  ],
  [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "a8ae722a78e10cbbc413886c02eb5b369a03f6560084aff566bd597bb7ad8c1ccd86e81296852359bf2faddb5153c0a7445722987875e74287adac21adebe952",
  ],
  [
    millionAs,
    "3c3a876da14034ab60627c077bb98f7e120a2a5370212dffb3385a18d4f38859ed311d0a9d5141ce9cc5c66ee689b266a8aa18ace8282a0e0db596c90b0a7b87",
  ],
];

const testSetKeccak224 = [
  ["", "f71837502ba8e10837bdd8d365adb85591895602fc552b48b7390abd"],
  ["abc", "c30411768506ebe1c2871b1ee2e87d38df342317300a9b97a95ec6a8"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "e51faa2b4655150b931ee8d700dc202f763ca5f962c529eae55012b6",
  ],
  [millionAs, "19f9167be2a04c43abd0ed554788101b9c339031acc8e1468531303f"],
];

const testSetKeccak256 = [
  ["", "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
  ["abc", "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "45d3b367a6904e6e8d502ee04999a7c27647f91fa845d456525fd352ae3d7371",
  ],
  [
    millionAs,
    "fadae6b49f129bbb812be8407b7b2894f34aecf6dbd1f9b0f0c7e9853098fc96",
  ],
];

const testSetKeccak384 = [
  [
    "",
    "2c23146a63a29acf99e73b88f8c24eaa7dc60aa771780ccc006afbfa8fe2479b2dd2b21362337441ac12b515911957ff",
  ],
  [
    "abc",
    "f7df1165f033337be098e7d288ad6a2f74409d7a60b49c36642218de161b1f99f8c681e4afaf31a34db29fb763e3c28e",
  ],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "b41e8896428f1bcbb51e17abd6acc98052a3502e0d5bf7fa1af949b4d3c855e7c4dc2c390326b3f3e74c7b1e2b9a3657",
  ],
  [
    millionAs,
    "0c8324e1ebc182822c5e2a086cac07c2fe00e3bce61d01ba8ad6b71780e2dec5fb89e5ae90cb593e57bc6258fdd94e17",
  ],
];

const testSetKeccak512 = [
  [
    "",
    "0eab42de4c3ceb9235fc91acffe746b29c29a8c366b7c60e4e67c466f36a4304c00fa9caf9d87976ba469bcbe06713b435f091ef2769fb160cdab33d3670680e",
  ],
  [
    "abc",
    "18587dc2ea106b9a1563e32b3312421ca164c7f1f07bc922a9c83d77cea3a1e5d0c69910739025372dc14ac9642629379540c17e2a65b19d77aa511a9d00bb96",
  ],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "6aa6d3669597df6d5a007b00d09c20795b5c4218234e1698a944757a488ecdc09965435d97ca32c3cfed7201ff30e070cd947f1fc12b9d9214c467d342bcba5d",
  ],
  [
    millionAs,
    "5cf53f2e556be5a624425ede23d0e8b2c7814b4ba0e4e09cbbf3c2fac7056f61e048fc341262875ebc58a5183fea651447124370c1ebf4d6c89bc9a7731063bb",
  ],
];

const testSetShake128 = [
  ["", "7f9c2ba4e88f827d616045507605853e"],
  ["abc", "5881092dd818bf5cf8a3ddb793fbcba7"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "1a96182b50fb8c7e74e0a707788f55e9",
  ],
  [millionAs, "9d222c79c4ff9d092cf6ca86143aa411"],
];

const testSetShake128_224 = [
  ["", "7f9c2ba4e88f827d616045507605853ed73b8093f6efbc88eb1a6eac"],
  ["abc", "5881092dd818bf5cf8a3ddb793fbcba74097d5c526a6d35f97b83351"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "1a96182b50fb8c7e74e0a707788f55e98209b8d91fade8f32f8dd5cf",
  ],
  [millionAs, "9d222c79c4ff9d092cf6ca86143aa411e369973808ef97093255826c"],
];

const testSetShake128_2048 = [
  [
    "",
    "7f9c2ba4e88f827d616045507605853ed73b8093f6efbc88eb1a6eacfa66ef263cb1eea988004b93103cfb0aeefd2a686e01fa4a58e8a3639ca8a1e3f9ae57e235b8cc873c23dc62b8d260169afa2f75ab916a58d974918835d25e6a435085b2badfd6dfaac359a5efbb7bcc4b59d538df9a04302e10c8bc1cbf1a0b3a5120ea17cda7cfad765f5623474d368ccca8af0007cd9f5e4c849f167a580b14aabdefaee7eef47cb0fca9767be1fda69419dfb927e9df07348b196691abaeb580b32def58538b8d23f87732ea63b02b4fa0f4873360e2841928cd60dd4cee8cc0d4c922a96188d032675c8ac850933c7aff1533b94c834adbb69c6115bad4692d8619",
  ],
  [
    "abc",
    "5881092dd818bf5cf8a3ddb793fbcba74097d5c526a6d35f97b83351940f2cc844c50af32acd3f2cdd066568706f509bc1bdde58295dae3f891a9a0fca5783789a41f8611214ce612394df286a62d1a2252aa94db9c538956c717dc2bed4f232a0294c857c730aa16067ac1062f1201fb0d377cfb9cde4c63599b27f3462bba4a0ed296c801f9ff7f57302bb3076ee145f97a32ae68e76ab66c48d51675bd49acc29082f5647584e6aa01b3f5af057805f973ff8ecb8b226ac32ada6f01c1fcd4818cb006aa5b4cdb3611eb1e533c8964cacfdf31012cd3fb744d02225b988b475375faad996eb1b9176ecb0f8b2871723d6dbb804e23357e50732f5cfc904b1",
  ],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "1a96182b50fb8c7e74e0a707788f55e98209b8d91fade8f32f8dd5cff7bf21f54ee5f19550825a6e070030519e944263ac1c6765287065621f9fcb3201723e3223b63a46c2938aa953ba8401d0ea77b8d26490775566407b95673c0f4cc1ce9fd966148d7efdff26bbf9f48a21c6da35bfaa545654f70ae586ff10131420771483ec92edab408c767bf4c5b4fffaa80c8ca214d84c4dc700d0c50630b2ffc3793ea4d87258b4c9548c5485a5ca666ef73fbd816d418aea6395b503addd9b150f9e0663325f01e5518b71ffa1244ea284cebe0cea2f774d7b3a437dca3282e324777e19624bf2be3cd355c1bfbddb323a33f11efafb2448293501dc0454c6b72f",
  ],
  [
    millionAs,
    "9d222c79c4ff9d092cf6ca86143aa411e369973808ef97093255826c5572ef58424c4b5c28475ffdcf981663867fec6321c1262e387bccf8ca676884c4a9d0c13bfa6869763d5ae4bbc9b3ccd09d1ca5ea7446538d69b3fb98c72b59a2b4817db5eadd9011f90fa71091931f8134f4f00b562e2fe105937270361c1909862ad45046e3932f5dd311ec72fec5f8fb8f60b45a3bee3f85bbf7fcedc6a555677648e0654b381941a86bd3e512657b0d57a7991fc4543f89d8290492222ce4a33e17602b3b99c009f7655f87535cdaa3716f58c47b8a157ad195f02809f27500b9254979311c6bb415968cd10431169a27d5a8d61e13a6b8b77af1f8b6dd2eefdea0",
  ],
];

const testSetShake256 = [
  ["", "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762f"],
  ["abc", "483366601360a8771c6863080cc4114d8db44530f8f1e1ee4f94ea37e78b5739"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "4d8c2dd2435a0128eefbb8c36f6f87133a7911e18d979ee1ae6be5d4fd2e3329",
  ],
  [
    millionAs,
    "3578a7a4ca9137569cdf76ed617d31bb994fca9c1bbf8b184013de8234dfd13a",
  ],
];

const testSetShake256_128 = [
  ["", "46b9dd2b0ba88d13233b3feb743eeb24"],
  ["abc", "483366601360a8771c6863080cc4114d"],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "4d8c2dd2435a0128eefbb8c36f6f8713",
  ],
  [millionAs, "3578a7a4ca9137569cdf76ed617d31bb"],
];

const testSetShake256_384 = [
  [
    "",
    "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762fd75dc4ddd8c0f200cb05019d67b592f6",
  ],
  [
    "abc",
    "483366601360a8771c6863080cc4114d8db44530f8f1e1ee4f94ea37e78b5739d5a15bef186a5386c75744c0527e1faa",
  ],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "4d8c2dd2435a0128eefbb8c36f6f87133a7911e18d979ee1ae6be5d4fd2e332940d8688a4e6a59aa8060f1f9bc996c05",
  ],
  [
    millionAs,
    "3578a7a4ca9137569cdf76ed617d31bb994fca9c1bbf8b184013de8234dfd13a3fd124d4df76c0a539ee7dd2f6e1ec34",
  ],
];

const testSetShake256_512 = [
  [
    "",
    "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762fd75dc4ddd8c0f200cb05019d67b592f6fc821c49479ab48640292eacb3b7c4be",
  ],
  [
    "abc",
    "483366601360a8771c6863080cc4114d8db44530f8f1e1ee4f94ea37e78b5739d5a15bef186a5386c75744c0527e1faa9f8726e462a12a4feb06bd8801e751e4",
  ],
  [
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "4d8c2dd2435a0128eefbb8c36f6f87133a7911e18d979ee1ae6be5d4fd2e332940d8688a4e6a59aa8060f1f9bc996c05aca3c696a8b66279dc672c740bb224ec",
  ],
  [
    millionAs,
    "3578a7a4ca9137569cdf76ed617d31bb994fca9c1bbf8b184013de8234dfd13a3fd124d4df76c0a539ee7dd2f6e1ec346124c815d9410e145eb561bcd97b18ab",
  ],
];

function s2b(data: string): Uint8Array {
  return new TextEncoder().encode(data);
}

Deno.test("[hash/sha3] testSha3-224Raw", () => {
  const sha3sum = (data: ArrayBuffer): ArrayBuffer => {
    const sha3 = new Sha3_224();
    return sha3.update(data).digest();
  };

  for (const [input, output] of testSetSha3_224) {
    const rawOutput = hex.decodeString(output);
    assertEquals(sha3sum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testSha3-224String", () => {
  const sha3sum = (data: string): string => {
    const sha3 = new Sha3_224();
    return sha3.update(data).toString();
  };

  for (const [input, output] of testSetSha3_224) {
    assertEquals(sha3sum(input), output);
  }
});

Deno.test("[hash/sha3] testSha3-256Raw", () => {
  const sha3sum = (data: ArrayBuffer): ArrayBuffer => {
    const sha3 = new Sha3_256();
    return sha3.update(data).digest();
  };

  for (const [input, output] of testSetSha3_256) {
    const rawOutput = hex.decodeString(output);
    assertEquals(sha3sum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testSha3-256String", () => {
  const sha3sum = (data: string): string => {
    const sha3 = new Sha3_256();
    return sha3.update(data).toString();
  };

  for (const [input, output] of testSetSha3_256) {
    assertEquals(sha3sum(input), output);
  }
});

Deno.test("[hash/sha3] testSha3-384Raw", () => {
  const sha3sum = (data: ArrayBuffer): ArrayBuffer => {
    const sha3 = new Sha3_384();
    return sha3.update(data).digest();
  };

  for (const [input, output] of testSetSha3_384) {
    const rawOutput = hex.decodeString(output);
    assertEquals(sha3sum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testSha3-384String", () => {
  const sha3sum = (data: string): string => {
    const sha3 = new Sha3_384();
    return sha3.update(data).toString();
  };

  for (const [input, output] of testSetSha3_384) {
    assertEquals(sha3sum(input), output);
  }
});

Deno.test("[hash/sha3] testSha3-512Raw", () => {
  const sha3sum = (data: ArrayBuffer): ArrayBuffer => {
    const sha3 = new Sha3_512();
    return sha3.update(data).digest();
  };

  for (const [input, output] of testSetSha3_512) {
    const rawOutput = hex.decodeString(output);
    assertEquals(sha3sum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testSha3-512String", () => {
  const sha3sum = (data: string): string => {
    const sha3 = new Sha3_512();
    return sha3.update(data).toString();
  };

  for (const [input, output] of testSetSha3_512) {
    assertEquals(sha3sum(input), output);
  }
});

Deno.test("[hash/sha3] testKeccak-224Raw", () => {
  const keccakSum = (data: ArrayBuffer): ArrayBuffer => {
    const keccak = new Keccak224();
    return keccak.update(data).digest();
  };

  for (const [input, output] of testSetKeccak224) {
    const rawOutput = hex.decodeString(output);
    assertEquals(keccakSum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testKeccak-224String", () => {
  const keccakSum = (data: string): string => {
    const keccak = new Keccak224();
    return keccak.update(data).toString();
  };

  for (const [input, output] of testSetKeccak224) {
    assertEquals(keccakSum(input), output);
  }
});

Deno.test("[hash/sha3] testKeccak-256Raw", () => {
  const keccakSum = (data: ArrayBuffer): ArrayBuffer => {
    const keccak = new Keccak256();
    return keccak.update(data).digest();
  };

  for (const [input, output] of testSetKeccak256) {
    const rawOutput = hex.decodeString(output);
    assertEquals(keccakSum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testKeccak-256String", () => {
  const keccakSum = (data: string): string => {
    const keccak = new Keccak256();
    return keccak.update(data).toString();
  };

  for (const [input, output] of testSetKeccak256) {
    assertEquals(keccakSum(input), output);
  }
});

Deno.test("[hash/sha3] testKeccak-384Raw", () => {
  const keccakSum = (data: ArrayBuffer): ArrayBuffer => {
    const keccak = new Keccak384();
    return keccak.update(data).digest();
  };

  for (const [input, output] of testSetKeccak384) {
    const rawOutput = hex.decodeString(output);
    assertEquals(keccakSum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testKeccak-384String", () => {
  const keccakSum = (data: string): string => {
    const keccak = new Keccak384();
    return keccak.update(data).toString();
  };

  for (const [input, output] of testSetKeccak384) {
    assertEquals(keccakSum(input), output);
  }
});

Deno.test("[hash/sha3] testKeccak-512Raw", () => {
  const keccakSum = (data: ArrayBuffer): ArrayBuffer => {
    const keccak = new Keccak512();
    return keccak.update(data).digest();
  };

  for (const [input, output] of testSetKeccak512) {
    const rawOutput = hex.decodeString(output);
    assertEquals(keccakSum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testKeccak-512String", () => {
  const keccakSum = (data: string): string => {
    const keccak = new Keccak512();
    return keccak.update(data).toString();
  };

  for (const [input, output] of testSetKeccak512) {
    assertEquals(keccakSum(input), output);
  }
});

Deno.test("[hash/sha3] testSHAKE-128Raw", () => {
  const shakeSum = (data: ArrayBuffer): ArrayBuffer => {
    const shake = new Shake128(128);
    return shake.update(data).digest();
  };

  for (const [input, output] of testSetShake128) {
    const rawOutput = hex.decodeString(output);
    assertEquals(shakeSum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testSHAKE-128String", () => {
  const shakeSum = (data: string): string => {
    const shake = new Shake128(128);
    return shake.update(data).toString();
  };

  for (const [input, output] of testSetShake128) {
    assertEquals(shakeSum(input), output);
  }
});

Deno.test("[hash/sha3] testSHAKE-128-224Raw", () => {
  const shakeSum = (data: ArrayBuffer): ArrayBuffer => {
    const shake = new Shake128(224);
    return shake.update(data).digest();
  };

  for (const [input, output] of testSetShake128_224) {
    const rawOutput = hex.decodeString(output);
    assertEquals(shakeSum(s2b(input)), rawOutput);
  }
});

Deno.test("[hash/sha3] testSHAKE-128-224String", () => {
  const shakeSum = (data: string): string => {
    const shake = new Shake128(224);
    return shake.update(data).toString();
  };

  for (const [input, output] of testSetShake128_224) {
    assertEquals(shakeSum(input), output);
  }
});

Deno.test("[hash/sha3] testSHAKE-128-2048", () => {
  const shakeSum = (data: string): string => {
    const shake = new Shake128(2048);
    return shake.update(data).toString();
  };

  for (const [input, output] of testSetShake128_2048) {
    assertEquals(shakeSum(input), output);
  }
});

Deno.test("[hash/sha3] testSHAKE-256", () => {
  const shakeSum = (data: string): string => {
    const shake = new Shake256(256);
    return shake.update(data).toString();
  };

  for (const [input, output] of testSetShake256) {
    assertEquals(shakeSum(input), output);
  }
});

Deno.test("[hash/sha3] testSHAKE-256-128", () => {
  const shakeSum = (data: string): string => {
    const shake = new Shake256(128);
    return shake.update(data).toString();
  };

  for (const [input, output] of testSetShake256_128) {
    assertEquals(shakeSum(input), output);
  }
});

Deno.test("[hash/sha3] testSHAKE-256-384", () => {
  const shakeSum = (data: string): string => {
    const shake = new Shake256(384);
    return shake.update(data).toString();
  };

  for (const [input, output] of testSetShake256_384) {
    assertEquals(shakeSum(input), output);
  }
});

Deno.test("[hash/sha3] testSHAKE-256-512", () => {
  const shakeSum = (data: string): string => {
    const shake = new Shake256(512);
    return shake.update(data).toString();
  };

  for (const [input, output] of testSetShake256_512) {
    assertEquals(shakeSum(input), output);
  }
});

Deno.test("[hash/sha3] testSha3-256Chain", () => {
  const sha3 = new Sha3_256();
  const output = sha3
    .update(s2b("a"))
    .update(s2b("b"))
    .update(s2b("c"))
    .toString();

  assertEquals(
    output,
    "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
  );
});

Deno.test("[hash/sha3] testSha3UpdateFinalized", () => {
  assertThrows(
    () => {
      const sha3 = new Sha3_256();
      const hash = sha3.update(s2b("a")).digest();
      const hash2 = sha3.update(s2b("a")).digest();
      assertEquals(hash, hash2);
    },
    Error,
    "sha3: cannot update already finalized hash"
  );
});
