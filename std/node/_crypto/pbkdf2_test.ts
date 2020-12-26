import {
  NormalizedAlgorithms as Algorithms,
  pbkdf2,
  pbkdf2Sync,
} from "./pbkdf2.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../../testing/asserts.ts";

type Pbkdf2Fixture = {
  key: string | Float64Array | Int32Array | Uint8Array;
  salt: string | Float64Array | Int32Array | Uint8Array;
  iterations: number;
  dkLen: number;
  results: { [key in Algorithms]: string };
};

const fixtures: Pbkdf2Fixture[] = [
  {
    "key": "password",
    "salt": "salt",
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "f31afb6d931392daa5e3130f47f9a9b6e8e72029d8350b9fb27a9e0e00b9d991",
      "sha1":
        "0c60c80f961f0e71f3a9b524af6012062fe037a6e0f0eb94fe8fc46bdc637164",
      "sha256":
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b",
      "sha512":
        "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252",
      "sha224":
        "3c198cbdb9464b7857966bd05b7bc92bc1cc4e6e63155d4e490557fd85989497",
      "sha384":
        "c0e14f06e49e32d73f9f52ddf1d0c5c7191609233631dadd76a567db42b78676",
      "ripemd160":
        "b725258b125e0bacb0e2307e34feb16a4d0d6aed6cb4b0eee458fc1829020428",
    },
  },
  {
    "key": "password",
    "salt": "salt",
    "iterations": 2,
    "dkLen": 32,
    "results": {
      "md5": "042407b552be345ad6eee2cf2f7ed01dd9662d8f0c6950eaec7124aa0c82279e",
      "sha1":
        "ea6c014dc72d6f8ccd1ed92ace1d41f0d8de8957cae93136266537a8d7bf4b76",
      "sha256":
        "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43",
      "sha512":
        "e1d9c16aa681708a45f5c7c4e215ceb66e011a2e9f0040713f18aefdb866d53c",
      "sha224":
        "93200ffa96c5776d38fa10abdf8f5bfc0054b9718513df472d2331d2d1e66a3f",
      "sha384":
        "54f775c6d790f21930459162fc535dbf04a939185127016a04176a0730c6f1f4",
      "ripemd160":
        "768dcc27b7bfdef794a1ff9d935090fcf598555e66913180b9ce363c615e9ed9",
    },
  },
  {
    "key": "password",
    "salt": "salt",
    "iterations": 1,
    "dkLen": 64,
    "results": {
      "md5":
        "f31afb6d931392daa5e3130f47f9a9b6e8e72029d8350b9fb27a9e0e00b9d9915a5f18928639ca8bbc3d1c1cb66d4f27b9dfe39156774c6798b42adc57ed253f",
      "sha1":
        "0c60c80f961f0e71f3a9b524af6012062fe037a6e0f0eb94fe8fc46bdc637164ac2e7a8e3f9d2e83ace57e0d50e5e1071367c179bc86c767fc3f78ddb561363f",
      "sha256":
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b4dbf3a2f3dad3377264bb7b8e8330d4efc7451418617dabef683735361cdc18c",
      "sha512":
        "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252c02d470a285a0501bad999bfe943c08f050235d7d68b1da55e63f73b60a57fce",
      "sha224":
        "3c198cbdb9464b7857966bd05b7bc92bc1cc4e6e63155d4e490557fd859894978ab846d52a1083ac610c36c2c5ea8ce4a024dd691064d5453bd17b15ea1ac194",
      "sha384":
        "c0e14f06e49e32d73f9f52ddf1d0c5c7191609233631dadd76a567db42b78676b38fc800cc53ddb642f5c74442e62be44d727702213e3bb9223c53b767fbfb5d",
      "ripemd160":
        "b725258b125e0bacb0e2307e34feb16a4d0d6aed6cb4b0eee458fc18290204289e55d962783bf52237d264cbbab25f18d89d8c798f90f558ea7b45bdf3d08334",
    },
  },
  {
    "key": "password",
    "salt": "salt",
    "iterations": 2,
    "dkLen": 64,
    "results": {
      "md5":
        "042407b552be345ad6eee2cf2f7ed01dd9662d8f0c6950eaec7124aa0c82279ed0b7e2a854d0f29ec82ddcabe9760368e5821af8745d74846ccbd17afbfe5ff0",
      "sha1":
        "ea6c014dc72d6f8ccd1ed92ace1d41f0d8de8957cae93136266537a8d7bf4b76c51094cc1ae010b19923ddc4395cd064acb023ffd1edd5ef4be8ffe61426c28e",
      "sha256":
        "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43830651afcb5c862f0b249bd031f7a67520d136470f5ec271ece91c07773253d9",
      "sha512":
        "e1d9c16aa681708a45f5c7c4e215ceb66e011a2e9f0040713f18aefdb866d53cf76cab2868a39b9f7840edce4fef5a82be67335c77a6068e04112754f27ccf4e",
      "sha224":
        "93200ffa96c5776d38fa10abdf8f5bfc0054b9718513df472d2331d2d1e66a3f97b510224f700ce72581ffb10a1c99ec99a8cc1b951851a71f30d9265fccf912",
      "sha384":
        "54f775c6d790f21930459162fc535dbf04a939185127016a04176a0730c6f1f4fb48832ad1261baadd2cedd50814b1c806ad1bbf43ebdc9d047904bf7ceafe1e",
      "ripemd160":
        "768dcc27b7bfdef794a1ff9d935090fcf598555e66913180b9ce363c615e9ed953b95fd07169be535e38afbea29c030e06d14f40745b1513b7ccdf0e76229e50",
    },
  },
  {
    "key": "password",
    "salt": "salt",
    "iterations": 4096,
    "dkLen": 32,
    "results": {
      "md5": "15001f89b9c29ee6998c520d1a0629e893cc3f996a08d27060e4c33305bf0fb2",
      "sha1":
        "4b007901b765489abead49d926f721d065a429c12e463f6c4cd79401085b03db",
      "sha256":
        "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a",
      "sha512":
        "d197b1b33db0143e018b12f3d1d1479e6cdebdcc97c5c0f87f6902e072f457b5",
      "sha224":
        "218c453bf90635bd0a21a75d172703ff6108ef603f65bb821aedade1d6961683",
      "sha384":
        "559726be38db125bc85ed7895f6e3cf574c7a01c080c3447db1e8a76764deb3c",
      "ripemd160":
        "99a40d3fe4ee95869791d9faa24864562782762171480b620ca8bed3dafbbcac",
    },
  },
  {
    "key": "passwordPASSWORDpassword",
    "salt": "saltSALTsaltSALTsaltSALTsaltSALTsalt",
    "iterations": 4096,
    "dkLen": 40,
    "results": {
      "md5":
        "8d5d0aad94d14420429fbc7e5b087d7a5527e65dfd0d486a310e8a7b6ff5a21bed000b118b2c26a6",
      "sha1":
        "3d2eec4fe41c849b80c8d83662c0e44a8b291a964cf2f07038b6b89a48612c5a25284e6605e12329",
      "sha256":
        "348c89dbcbd32b2f32d814b8116e84cf2b17347ebc1800181c4e2a1fb8dd53e1c635518c7dac47e9",
      "sha512":
        "8c0511f4c6e597c6ac6315d8f0362e225f3c501495ba23b868c005174dc4ee71115b59f9e60cd953",
      "sha224":
        "056c4ba438ded91fc14e0594e6f52b87e1f3690c0dc0fbc05784ed9a754ca780e6c017e80c8de278",
      "sha384":
        "819143ad66df9a552559b9e131c52ae6c5c1b0eed18f4d283b8c5c9eaeb92b392c147cc2d2869d58",
      "ripemd160":
        "503b9a069633b261b2d3e4f21c5d0cafeb3f5008aec25ed21418d12630b6ce036ec82a0430ef1974",
    },
  },
  {
    "key": "pass\u00000word",
    "salt": "sa\u00000lt",
    "iterations": 4096,
    "dkLen": 16,
    "results": {
      "md5": "2d6b566fd00069a30dd1ffdb4d598f54",
      "sha1": "345cbad979dfccb90cac5257bea6ea46",
      "sha256": "1df6274d3c0bd2fc7f54fb46f149dda4",
      "sha512": "336d14366099e8aac2c46c94a8f178d2",
      "sha224": "0aca9ca9634db6ef4927931f633c6453",
      "sha384": "b6ab6f8f6532fd9c5c30a79e1f93dcc6",
      "ripemd160": "914d58209e6483e491571a60e433124a",
    },
  },
  {
    "key": "63ffeeddccbbaa",
    "salt": "salt",
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "23a33e38c9c57ea122df372a5b96347667e843ba21c79f150ce503d947449b75",
      "sha1":
        "1a12b21aa46bd3bed3a23b8ad072a1465585344b1516252618aabbc41276dada",
      "sha256":
        "a47c9371d4d4f663c2a0d3becbd475b7eb884722c7265391381d7696151470a6",
      "sha512":
        "09328469e02fcee4f6ab88a23037de33d54f17f786eee39e1f8826109ee54e16",
      "sha224":
        "59baceb002865e57061c65dd861c309c049a97207054416c943764efc38b94ed",
      "sha384":
        "01cc52b81eda47c8bc9861ab7f7de682e92a0d5e522f4d3a06a3b97be1856580",
      "ripemd160":
        "4f04f4782f2def250005e04ef0497403330b52a085ae856f4640700b19983b7c",
    },
  },
  {
    "key":
      "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "salt":
      "6d6e656d6f6e6963e383a1e383bce38388e383abe382abe38299e3838fe38299e382a6e38299e382a1e381afe3829ae381afe38299e3818fe38299e3829de38299e381a1e381a1e38299e58d81e4babae58d81e889b2",
    "iterations": 2048,
    "dkLen": 64,
    "results": {
      "md5":
        "029d0e4484f56d9cf7ab7ca972c8991aeb2be275cba9683db4143e9b72f67d49551ec4c70ca6d051538fc7a86b8568d08244fdea24ba826b7927babac4f62cf2",
      "sha1":
        "fb3fa7c05a98ff66da2eadd69fa2ba52401ee630e04322d3c5bb018d1dda03c7e47bdea0c9e4c77c87826632eed59bbe42ce05329a838664683b1a8dae3fffd8",
      "sha256":
        "3b19907cb907d1ee6e5a0ecb80bd66e2776d1f2c73f4789eafcad94fda832e970471ceb0d200ede70e63ae021044cf4b58b1011e34252ace8d94a48c287906ec",
      "sha512":
        "0be4563c5175fd02b042251228774f34c1ccb235054a9f0f968c6d828466eae8c32433a7aa09ce922722dc808c6a1629ba8f1b6ba46f0cf7a921e125d1cc9fcd",
      "sha224":
        "dd529ad11b298cafad9209a0a620af98cf1b782bd0ba1a61efcd74a4fe2662af6c36ffd015c68ed0cd630bdb023ea61e59317eb07b342e0c6ece1bd3034b768c",
      "sha384":
        "7265c090b602b0a432b4908f70b6a5a2a6657926d09ac72ebb78d8bcc81e0d4563316f1eb5570b2850ef06a14719746a8a8397d3d56aa51b2d50489741b7ff61",
      "ripemd160":
        "c984beaf664aea5ae7f671063ef2ad1f80098e48382a916809ff9212d1a8cb7ad6cb17354422717c668726dfce294e1442bb354b6a6693db84032172e77af6ae",
    },
  },
  {
    "key": "password",
    "salt": "salt",
    "iterations": 1,
    "dkLen": 10,
    "results": {
      "md5": "f31afb6d931392daa5e3",
      "sha1": "0c60c80f961f0e71f3a9",
      "sha256": "120fb6cffcf8b32c43e7",
      "sha512": "867f70cf1ade02cff375",
      "sha224": "3c198cbdb9464b785796",
      "sha384": "c0e14f06e49e32d73f9f",
      "ripemd160": "b725258b125e0bacb0e2",
    },
  },
  {
    "key": "password",
    "salt": "salt",
    "iterations": 1,
    "dkLen": 100,
    "results": {
      "md5":
        "f31afb6d931392daa5e3130f47f9a9b6e8e72029d8350b9fb27a9e0e00b9d9915a5f18928639ca8bbc3d1c1cb66d4f27b9dfe39156774c6798b42adc57ed253f44fc731edccf067904ce2e317b9ef45767add4dfe53f8c190dac43d90cda5e66e627d4f2",
      "sha1":
        "0c60c80f961f0e71f3a9b524af6012062fe037a6e0f0eb94fe8fc46bdc637164ac2e7a8e3f9d2e83ace57e0d50e5e1071367c179bc86c767fc3f78ddb561363fc692ba406d1301e42bcccc3c520d06751d78b80c3db926b16ffa3395bd697c647f280b51",
      "sha256":
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b4dbf3a2f3dad3377264bb7b8e8330d4efc7451418617dabef683735361cdc18c22cd7fe60fa40e91c65849e1f60c0d8b62a7b2dbd0d3dfd75fb8498a5c2131ab02b66de5",
      "sha512":
        "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252c02d470a285a0501bad999bfe943c08f050235d7d68b1da55e63f73b60a57fce7b532e206c2967d4c7d2ffa460539fc4d4e5eec70125d74c6c7cf86d25284f297907fcea",
      "sha224":
        "3c198cbdb9464b7857966bd05b7bc92bc1cc4e6e63155d4e490557fd859894978ab846d52a1083ac610c36c2c5ea8ce4a024dd691064d5453bd17b15ea1ac1944bbfd62e61b997e7b22660f588e297186572480015f33bc2bfd2b423827bcdcdb4845914",
      "sha384":
        "c0e14f06e49e32d73f9f52ddf1d0c5c7191609233631dadd76a567db42b78676b38fc800cc53ddb642f5c74442e62be44d727702213e3bb9223c53b767fbfb5db9d270d54c45d9cb6003d2967280b22671e2dbc6375f6ebf219c36f0d127be35e19d65a8",
      "ripemd160":
        "b725258b125e0bacb0e2307e34feb16a4d0d6aed6cb4b0eee458fc18290204289e55d962783bf52237d264cbbab25f18d89d8c798f90f558ea7b45bdf3d083340c18b9d23ba842183c5364d18bc0ffde5a8a408dd7ef02dde561a08d21c6d2325a69869b",
    },
  },
  {
    "key": new Uint8Array([112, 97, 115, 115, 119, 111, 114, 100]),
    "salt": "salt",
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "f31afb6d931392daa5e3130f47f9a9b6e8e72029d8350b9fb27a9e0e00b9d991",
      "sha1":
        "0c60c80f961f0e71f3a9b524af6012062fe037a6e0f0eb94fe8fc46bdc637164",
      "sha256":
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b",
      "sha512":
        "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252",
      "sha224":
        "3c198cbdb9464b7857966bd05b7bc92bc1cc4e6e63155d4e490557fd85989497",
      "sha384":
        "c0e14f06e49e32d73f9f52ddf1d0c5c7191609233631dadd76a567db42b78676",
      "ripemd160":
        "b725258b125e0bacb0e2307e34feb16a4d0d6aed6cb4b0eee458fc1829020428",
    },
  },
  {
    "key": "password",
    "salt": new Uint8Array([115, 97, 108, 116]),
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "f31afb6d931392daa5e3130f47f9a9b6e8e72029d8350b9fb27a9e0e00b9d991",
      "sha1":
        "0c60c80f961f0e71f3a9b524af6012062fe037a6e0f0eb94fe8fc46bdc637164",
      "sha256":
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b",
      "sha512":
        "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252",
      "sha224":
        "3c198cbdb9464b7857966bd05b7bc92bc1cc4e6e63155d4e490557fd85989497",
      "sha384":
        "c0e14f06e49e32d73f9f52ddf1d0c5c7191609233631dadd76a567db42b78676",
      "ripemd160":
        "b725258b125e0bacb0e2307e34feb16a4d0d6aed6cb4b0eee458fc1829020428",
    },
  },
  {
    "key": new Int32Array([112, 97, 115, 115, 119, 111, 114, 100]),
    "salt": "salt",
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "81de8e85b07d7969d9fe530641b63cc4ecbbf2345037cdc0ba61ad329fc7029c",
      "sha1":
        "f260ccd0bbc8fe6773119b834feec48636b716caad4180a4d0af4f9aa67c646e",
      "sha256":
        "9b4608f5eeab348f0b9d85a918b140706b24f275acf6829382dfee491015f9eb",
      "sha512":
        "c44b8f26550fe6ca0a55bce54b4a75e9530398f32ec28b59d0fded996e95e3d5",
      "sha224":
        "03d0c2b530ec6339e6418cb0f906e50591619be40aa8817aa9c7305d1773231c",
      "sha384":
        "2e69d62ae8c21ebc2de45a885b488f65fb88dfa58aaa9c57dd1fcb9d1edce96a",
      "ripemd160":
        "fc69276ba3f145492065feb0259b9edf68179f2023c95094e71ac7d01748018a",
    },
  },
  {
    "key": "password",
    "salt": new Int32Array([115, 97, 108, 116]),
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "36587a57770a8eef264391786b4ddfae0723f6a64dc2fc199fe7eb6ad9def701",
      "sha1":
        "b297f1ea23008f10ba9d645961e4661109e804b10af26bea22c44244492d6252",
      "sha256":
        "f678f0772894c079f21377d9ee1e76dd77b62dfc1f0575e6aa9eb030af7a356a",
      "sha512":
        "7f8133f6937ae1d7e4a43c19aabd2de8308d5b833341281716a501334cdb2470",
      "sha224":
        "ab66d29d3dacc731e44f091a7baa051926219cf493e8b9e3934cedfb215adc8b",
      "sha384":
        "cf139d648cf63e9b85a3b9b8f23f4445b84d22201bc2544bc273a17d5dcb7b28",
      "ripemd160":
        "26142e48fae1ad1c53be54823aadda2aa7d42f5524463fb1eff0efafa08edb9d",
    },
  },
  {
    "key": new Float64Array([112, 97, 115, 115, 119, 111, 114, 100]),
    "salt": "salt",
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "48336072da7d11ff203c61705b384b1c60953e7d1677fed2cd3e65738d60e67e",
      "sha1":
        "c2b17a7e98cc48690a92cd9f753a2c700229045905167571aa281aafe8230bba",
      "sha256":
        "55d62579a083a6c14b886710f81b54f567d214d343af776e5e90c467ea81b821",
      "sha512":
        "ded01ce343e2683d962fc74b7b5ceef525228f49393ce9353254f44e3dc7e9aa",
      "sha224":
        "5f10a348d320c7555b972b8d7d45a363a91e1a82dea063c3ac495cfad74a8d89",
      "sha384":
        "4b7f97dbadfd652e0579499d0e23607ec476ed4bea9d6f1740d0b110e2d08792",
      "ripemd160":
        "f92080d972a649d98d91a53922863fc7b8076c54869e9885f9a804868ef752e0",
    },
  },
  {
    "key": "password",
    "salt": new Float64Array([115, 97, 108, 116]),
    "iterations": 1,
    "dkLen": 32,
    "results": {
      "md5": "9f1716e6f9d77b0beb56758f9509edea50828d15909073c3c715f66173ac3716",
      "sha1":
        "f158b9edd28c16ad3b41e0e8197ec132a98c2ddea73b959f55ec9792e0b29d6f",
      "sha256":
        "a6154d17480547a10212f75883509842f88f2ca5d6c1a2419646e47342051852",
      "sha512":
        "b10c2ea742de7dd0525988761ee1733564c91380eeaa1b199f4fafcbf7144b0c",
      "sha224":
        "29b315ac30c7d5e1640ca0f9e27b68a794fb9f950b8dd117129824f103ffb9db",
      "sha384":
        "624b4ed6ad389b976fb7503e54a35109f249c29ac6eb8b56850152be21b3cb0e",
      "ripemd160":
        "8999b9280207bc9c76cf25327aa352da26a683fac7a2adff17a39dcc4f4c3b5b",
    },
  },
];

Deno.test("pbkdf2 hashes data correctly", () => {
  fixtures.forEach(({
    dkLen,
    iterations,
    key,
    results,
    salt,
  }) => {
    for (const algorithm in results) {
      pbkdf2(
        key,
        salt,
        iterations,
        dkLen,
        algorithm as Algorithms,
        (err, res) => {
          assert(!err);
          assertEquals(
            res?.toString("hex"),
            results[algorithm as Algorithms],
          );
        },
      );
    }
  });
});

Deno.test("pbkdf2Sync hashes data correctly", () => {
  fixtures.forEach(({
    dkLen,
    iterations,
    key,
    results,
    salt,
  }) => {
    for (const algorithm in results) {
      assertEquals(
        pbkdf2Sync(key, salt, iterations, dkLen, algorithm as Algorithms)
          .toString("hex"),
        results[algorithm as Algorithms],
      );
    }
  });
});

Deno.test("[std/node/crypto] pbkdf2 callback isn't called twice if error is thrown", async () => {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { pbkdf2 } from "${new URL("./pbkdf2.ts", import.meta.url).href}";

      pbkdf2("password", "salt", 1, 32, "sha1", (err) => {
        // If the bug is present and the callback is called again with an error,
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (!err) throw new Error("success");
      });`,
    ],
    stderr: "piped",
  });
  const status = await p.status();
  const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
  p.close();
  p.stderr.close();
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
