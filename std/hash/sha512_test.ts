// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { HmacSha512, Message, Sha512 } from "./sha512.ts";
import { assertEquals } from "../testing/asserts.ts";
import { dirname, fromFileUrl, join } from "../path/mod.ts";
import { resolvePath } from "../fs/mod.ts";

const moduleDir = dirname(fromFileUrl(import.meta.url));
const testdataDir = resolvePath(moduleDir, "testdata");

/** Handy function to convert an array/array buffer to a string of hex values. */
function toHexString(value: number[] | ArrayBuffer): string {
  const array = new Uint8Array(value);
  let hex = "";
  for (const v of array) {
    const c = v.toString(16);
    hex += c.length === 1 ? `0${c}` : c;
  }
  return hex;
}

// deno-fmt-ignore
const fixtures: {
  sha512bits224: Record<string, Record<string, Message>>,
  sha512bits256: Record<string, Record<string, Message>>,
  sha512: Record<string, Record<string, Message>>,
  hmacSha512bits224: Record<string, Record<string, [Message, Message]>>,
  hmacSha512bits256: Record<string, Record<string, [Message, Message]>>,
  hmacSha512: Record<string, Record<string, [Message, Message]>>
} = {
  sha512bits224: {
    "ascii": {
      "6ed0dd02806fa89e25de060c19d3ac86cabb87d6a0ddd05c333b84f4": "",
      "944cd2847fb54558d4775db0485a50003111c8e5daa63fe722c6aa37": "The quick brown fox jumps over the lazy dog",
      "6d6a9279495ec4061769752e7ff9c68b6b0b3c5a281b7917ce0572de": "The quick brown fox jumps over the lazy dog."
    },
    "ascii more than 64 bytes": {
      "2e962464977b198ee758d615bbc92251ad2e3c0960068e279fd21d2f": "The MD5 message-digest algorithm is a widely used cryptographic hash function producing a 128-bit (16-byte) hash value, typically expressed in text format as a 32 digit hexadecimal number. MD5 has been utilized in a wide variety of cryptographic applications, and is also commonly used to verify data integrity."
    },
    "UTF8": {
      "0f46a0ae7f226517dd66ece0ce1efa29ffb7ced05ac4566fdcaed188": "中文",
      "562f2e4ee7f7451d20dcc6a0ac1a1e1c4a75f09baaf1cf19af3e15f4": "aécio",
      "0533318c52b3d4ad355c2a6c7e727ae3d2efa749db480ac33560b059": "𠜎"
    },
    "UTF8 more than 64 bytes": {
      "f67e191a5d4ee67a272ccaf6cf597f0c4d6a0c46bd631be7cadb0944": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一",
      "009c3d1e3172d6df71344982eada855421592aea28acbf660ada7569": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一（又譯雜湊演算法、摘要演算法等），主流程式語言普遍已有MD5的實作。"
    },
    "special length": {
      "6fe6ce0f03b9cd09851e05ba5e3103df56d2a3dbb379fee437e1cdd3": "0123456780123456780123456780123456780123456780123456780",
      "9e6994d879f14c242dea25ebc4d03ae6fc710f5eb60c3962b9dba797": "01234567801234567801234567801234567801234567801234567801",
      "204ce3b2af187fe90494cb3e4517257c44917bb7ea6578264baa4fcf": "0123456780123456780123456780123456780123456780123456780123456780",
      "69ce912fd1f87e02601d6153c02769ebd7c42b29dcb7963a1c3996da": "01234567801234567801234567801234567801234567801234567801234567801234567",
      "bd98be1f148dddd8a98c6ba31628c354456b9754166738fe1aba1037": "012345678012345678012345678012345678012345678012345678012345678012345678"
    },
    "Array": {
      "6ed0dd02806fa89e25de060c19d3ac86cabb87d6a0ddd05c333b84f4": [],
      "6945cf025ed66055282665c546781e32c5a479b5e9b479e96b0c23fe": [211, 212],
      "944cd2847fb54558d4775db0485a50003111c8e5daa63fe722c6aa37": [84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103],
      "69ce912fd1f87e02601d6153c02769ebd7c42b29dcb7963a1c3996da": [48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55]
    },
    "Uint8Array": {
      "6945cf025ed66055282665c546781e32c5a479b5e9b479e96b0c23fe": new Uint8Array([211, 212]),
      "944cd2847fb54558d4775db0485a50003111c8e5daa63fe722c6aa37": new Uint8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "Int8Array": {
      "944cd2847fb54558d4775db0485a50003111c8e5daa63fe722c6aa37": new Int8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "ArrayBuffer": {
      "6ed0dd02806fa89e25de060c19d3ac86cabb87d6a0ddd05c333b84f4": new ArrayBuffer(0),
      "283bb59af7081ed08197227d8f65b9591ffe1155be43e9550e57f941": new ArrayBuffer(1)
    }
  },
  sha512bits256: {
    "ascii": {
      "c672b8d1ef56ed28ab87c3622c5114069bdd3ad7b8f9737498d0c01ecef0967a": "",
      "dd9d67b371519c339ed8dbd25af90e976a1eeefd4ad3d889005e532fc5bef04d": "The quick brown fox jumps over the lazy dog",
      "1546741840f8a492b959d9b8b2344b9b0eb51b004bba35c0aebaac86d45264c3": "The quick brown fox jumps over the lazy dog."
    },
    "ascii more than 64 bytes": {
      "21e2e940930b23f1de6377086d07e22033c6bbf3fd9fbf4b62ec66e6c08c25be": "The MD5 message-digest algorithm is a widely used cryptographic hash function producing a 128-bit (16-byte) hash value, typically expressed in text format as a 32 digit hexadecimal number. MD5 has been utilized in a wide variety of cryptographic applications, and is also commonly used to verify data integrity."
    },
    "UTF8": {
      "b6dab29c16ec35ab34a5d92ff135b58de96741dda78b1009a2181cf8b45d2f72": "中文",
      "122802ca08e39c2ef46f6a81379dc5683bd8aa074dfb54259f0add4d8b5504bc": "aécio",
      "1032308151c0f4f5f8d4e0d96956352eb8ff87da98df8878d8795a858a7e7c08": "𠜎"
    },
    "UTF8 more than 64 bytes": {
      "d32a41d9858e45b68402f77cf9f3c3f992c36a4bffd230f78d666c87f97eaf7e": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一",
      "bd1abad59e6b8ad69bc17b6e05aa13f0cb725467fbeb45b83d3e4094332d1367": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一（又譯雜湊演算法、摘要演算法等），主流程式語言普遍已有MD5的實作。"
    },
    "special length": {
      "99fb09c8564fbd52274cfaf1130ae02dad89efac9a31dc00e9bfc13db1ff4f56": "0123456780123456780123456780123456780123456780123456780",
      "7a3204b58878f5a65a54f77e270d5df579a8016e0e472cc91833689c4cf8ca07": "01234567801234567801234567801234567801234567801234567801",
      "f4aa5f7692e6fee7237510b9a886f7b7aa4098926b45eaf70672bdd6d316a633": "0123456780123456780123456780123456780123456780123456780123456780",
      "3f8fc8ec35656592ce61bf44895b6d94077aae3bddd99236a0b04ccf936699ed": "01234567801234567801234567801234567801234567801234567801234567801234567",
      "4cb330a62170d92fe3d03bcf9284b590cf08d38d3a3c1e661abba3641d0b7502": "012345678012345678012345678012345678012345678012345678012345678012345678"
    },
    "Array": {
      "c672b8d1ef56ed28ab87c3622c5114069bdd3ad7b8f9737498d0c01ecef0967a": [],
      "547cf572033bb67ae341d010b348691ee9c550d07b796e0c6e6ad3503fa36cb3": [211, 212],
      "dd9d67b371519c339ed8dbd25af90e976a1eeefd4ad3d889005e532fc5bef04d": [84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103],
      "3f8fc8ec35656592ce61bf44895b6d94077aae3bddd99236a0b04ccf936699ed": [48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55]
    },
    "Uint8Array": {
      "547cf572033bb67ae341d010b348691ee9c550d07b796e0c6e6ad3503fa36cb3": new Uint8Array([211, 212]),
      "dd9d67b371519c339ed8dbd25af90e976a1eeefd4ad3d889005e532fc5bef04d": new Uint8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "Int8Array": {
      "dd9d67b371519c339ed8dbd25af90e976a1eeefd4ad3d889005e532fc5bef04d": new Int8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "ArrayBuffer": {
      "c672b8d1ef56ed28ab87c3622c5114069bdd3ad7b8f9737498d0c01ecef0967a": new ArrayBuffer(0),
      "10baad1713566ac2333467bddb0597dec9066120dd72ac2dcb8394221dcbe43d": new ArrayBuffer(1)
    }
  },
  sha512: {
    "ascii": {
      "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e": "",
      "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6": "The quick brown fox jumps over the lazy dog",
      "91ea1245f20d46ae9a037a989f54f1f790f0a47607eeb8a14d12890cea77a1bbc6c7ed9cf205e67b7f2b8fd4c7dfd3a7a8617e45f3c463d481c7e586c39ac1ed": "The quick brown fox jumps over the lazy dog."
    },
    "ascii more than 64 bytes": {
      "a8dedff31e3be9df6413ef5b4ecb93d62d3fbcb04297552eab5370e04afd45927854a4373037e81a50186e678d818c9ba824f4c850f3d0f02764af0252076979": "The MD5 message-digest algorithm is a widely used cryptographic hash function producing a 128-bit (16-byte) hash value, typically expressed in text format as a 32 digit hexadecimal number. MD5 has been utilized in a wide variety of cryptographic applications, and is also commonly used to verify data integrity."
    },
    "UTF8": {
      "8b88efc2ebbcbdad5ac2d65af05bec57bda25e71fd5fb25bbd892057a2755fbd05d8d8491cb2946febd5b0f124ffdfbaecf7e34946353c4f1b5ab29545895468": "中文",
      "e1c6925243db76985abacaf9fa85e22697f549e67f65a36c88e4046a2260990ff9eefc3402396ea8dcbe8c592d8d5671bea612156eda38d3708d394bbd17d493": "aécio",
      "f3e7ee9cdf7dbb52f7edd59ce3d49868c64f2b3aceceab060b8eaaebdf9de0dae5866d660e3319c5aad426a2176cb1703efc73eb24d1a90458ceda1b7f4e3940": "𠜎"
    },
    "UTF8 more than 64 bytes": {
      "6cb7f6d3381a187edadb43c7cdcfbbed4d2c213a7dce8ea08fe42b9882b64e643202b4974a6db94f94650ab9173d97c58bd59f6d19d27e01aab76d8d08855c65": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一",
      "d24af1901aaf1458f089a6eddf784ce61c3012aee0df98bdb67ad2dc6b41a3b4051d40caac524373930ae396a2dde99a9204871b40892eea3e5f3c8d46da0c3c": "訊息摘要演算法第五版（英語：Message-Digest Algorithm 5，縮寫為MD5），是當前電腦領域用於確保資訊傳輸完整一致而廣泛使用的雜湊演算法之一（又譯雜湊演算法、摘要演算法等），主流程式語言普遍已有MD5的實作。"
    },
    "special length": {
      "6b4a72eb22d2d24c0a429dd99ce5835b134144ac5fce446f66dbf2f421dcc5f8a177e4774f4a48173c5640724b186c2c4112a80937b1167f3e7bb511f4c41b6a": "0123456780123456780123456780123456780123456780123456780",
      "76f3cb2ed5b0b405479495b2d3576f4b469b6ffc4b06e3b512a658b84c1b91cf72c41c54d8714ecf19d04696f09e0034632fe98ae848ffd35b83c7e72399a590": "01234567801234567801234567801234567801234567801234567801",
      "56d2391faebd8d69b067cd5c0cb364ffc2e2ab87ce5bb06a562b44c8dcb0b83816ad2c0c062537838992b181fadc43ff00e1ebb92ddb1129b81b4864bafb5f63": "0123456780123456780123456780123456780123456780123456780123456780",
      "317ab88f192258711b8ae0197395b7a8191796fb41140c16c596699481149b47130e26b3bfa724227202fa8371752ca92e3cb9dd202caf29334038e0848cb43f": "01234567801234567801234567801234567801234567801234567801234567801234567",
      "23880e96199df52b4386d190adddaa33cbf7e0bfa7d2067c60eb44ee103667fd002c32e184195fef65fd4178853b1c661d9f260d721df85872e5f645f4388841": "012345678012345678012345678012345678012345678012345678012345678012345678"
    },
    "Array": {
      "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e": [],
      "8df0195b2807fdc8c7674c191562e9d0db38b257cc0d3df64669878fe5bb1bbaff53cc8898edcf46cbecb945dc71b6ad738da8ca6f3a824123a54afde5d1d5b0": [211, 212],
      "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6": [84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103],
      "317ab88f192258711b8ae0197395b7a8191796fb41140c16c596699481149b47130e26b3bfa724227202fa8371752ca92e3cb9dd202caf29334038e0848cb43f": [48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55, 56, 48, 49, 50, 51, 52, 53, 54, 55]
    },
    "Uint8Array": {
      "8df0195b2807fdc8c7674c191562e9d0db38b257cc0d3df64669878fe5bb1bbaff53cc8898edcf46cbecb945dc71b6ad738da8ca6f3a824123a54afde5d1d5b0": new Uint8Array([211, 212]),
      "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6": new Uint8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "Int8Array": {
      "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6": new Int8Array([84, 104, 101, 32, 113, 117, 105, 99, 107, 32, 98, 114, 111, 119, 110, 32, 102, 111, 120, 32, 106, 117, 109, 112, 115, 32, 111, 118, 101, 114, 32, 116, 104, 101, 32, 108, 97, 122, 121, 32, 100, 111, 103])
    },
    "ArrayBuffer": {
      "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e": new ArrayBuffer(0),
      "b8244d028981d693af7b456af8efa4cad63d282e19ff14942c246e50d9351d22704a802a71c3580b6370de4ceb293c324a8423342557d4e5c38438f0e36910ee": new ArrayBuffer(1)
    }
  },
  hmacSha512bits224: {
    "Test Vectors": {
      "b244ba01307c0e7a8ccaad13b1067a4cf6b961fe0c6a20bda3d92039": [
        [0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b],
        "Hi There"
      ],
      "4a530b31a79ebcce36916546317c45f247d83241dfb818fd37254bde": [
        "Jefe",
        "what do ya want for nothing?"
      ],
      "db34ea525c2c216ee5a6ccb6608bea870bbef12fd9b96a5109e2b6fc": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        [0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd]
      ],
      "c2391863cda465c6828af06ac5d4b72d0b792109952da530e11a0d26": [
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19],
        [0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd]
      ],
      "29bef8ce88b54d4226c3c7718ea9e32ace2429026f089e38cea9aeda": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "Test Using Larger Than Block-Size Key - Hash Key First"
      ],
      "82a9619b47af0cea73a8b9741355ce902d807ad87ee9078522a246e1": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm."
      ]
    },
    "UTF8": {
      "24e1153464bf5ec62ad2eeeb88ff644f2441a124d1e16e8ae5fb1508": ["中文", "aécio"],
      "7a08cecb4700304bc5c466acc1fb312d198374817052a03df07610c6": ["aécio", "𠜎"],
      "697973678b7d0075676ec3cbbc19e343ed16fa20c14d8074b76b0861": ["𠜎", "中文"]
    },
    "Uint8Array": {
      "defdc4a1a6597147ea0c7d0a59ae0a5e64b9413a6400acac28aecdd1": [new Uint8Array(0), "Hi There"]
    },
    "ArrayBuffer": {
      "defdc4a1a6597147ea0c7d0a59ae0a5e64b9413a6400acac28aecdd1": [new ArrayBuffer(0), "Hi There"]
    }
  },
  hmacSha512bits256: {
    "Test Vectors": {
      "9f9126c3d9c3c330d760425ca8a217e31feae31bfe70196ff81642b868402eab": [
        [0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b],
        "Hi There"
      ],
      "6df7b24630d5ccb2ee335407081a87188c221489768fa2020513b2d593359456": [
        "Jefe",
        "what do ya want for nothing?"
      ],
      "229006391d66c8ecddf43ba5cf8f83530ef221a4e9401840d1bead5137c8a2ea": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        [0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd]
      ],
      "36d60c8aa1d0be856e10804cf836e821e8733cbafeae87630589fd0b9b0a2f4c": [
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19],
        [0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd]
      ],
      "87123c45f7c537a404f8f47cdbedda1fc9bec60eeb971982ce7ef10e774e6539": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "Test Using Larger Than Block-Size Key - Hash Key First"
      ],
      "6ea83f8e7315072c0bdaa33b93a26fc1659974637a9db8a887d06c05a7f35a66": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm."
      ]
    },
    "UTF8": {
      "633400fa4bc12c3690efa218c90b56ab1af81b91ad62b57bdbe84988c51071e0": ["中文", "aécio"],
      "80eff00e32e0c0813d4c04e296b5ac079ec896e673cc04b0ff14222e151ad0b0": ["aécio", "𠜎"],
      "3f801c729e5330a0b91aecc751a26c35688a94989e2098c73bf0c6ac02b99e58": ["𠜎", "中文"]
    },
    "Uint8Array": {
      "1e08e33f9357abd2a3cfbc82a623d892bb6dccf175d22c0cf24269a7a59dfad6": [new Uint8Array(0), "Hi There"]
    },
    "ArrayBuffer": {
      "1e08e33f9357abd2a3cfbc82a623d892bb6dccf175d22c0cf24269a7a59dfad6": [new ArrayBuffer(0), "Hi There"]
    }
  },
  hmacSha512: {
    "Test Vectors": {
      "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854": [
        [0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b],
        "Hi There"
      ],
      "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737": [
        "Jefe",
        "what do ya want for nothing?"
      ],
      "fa73b0089d56a284efb0f0756c890be9b1b5dbdd8ee81a3655f83e33b2279d39bf3e848279a722c806b485a47e67c807b946a337bee8942674278859e13292fb": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        [0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd]
      ],
      "b0ba465637458c6990e5a8c5f61d4af7e576d97ff94b872de76f8050361ee3dba91ca5c11aa25eb4d679275cc5788063a5f19741120c4f2de2adebeb10a298dd": [
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19],
        [0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd, 0xcd]
      ],
      "80b24263c7c1a3ebb71493c1dd7be8b49b46d1f41b4aeec1121b013783f8f3526b56d037e05f2598bd0fd2215d6a1e5295e64f73f63f0aec8b915a985d786598": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "Test Using Larger Than Block-Size Key - Hash Key First"
      ],
      "e37b6a775dc87dbaa4dfa9f96e5e3ffddebd71f8867289865df5a32d20cdc944b6022cac3c4982b10d5eeb55c3e4de15134676fb6de0446065c97440fa8c6a58": [
        [0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
        "This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm."
      ]
    },
    "UTF8": {
      "e9e5906be0aecbc028a5fc759c9dbb86efc9a22950af8e678302a215aeee0b021edc50bbdd71c656730177b7e96c9a3bcf3cb9592bc84a5f3e8900cb67c7eca6": ["中文", "aécio"],
      "d02a8d258d855967d5be47240bbedd986a31c29eb5beb35abdbe2725651bf33a195cdfaadb9e76dc4790c71dfea33f708afa04b9471d03f5f0db8440993b9612": ["aécio", "𠜎"],
      "a443d463546586a5dd591ef848f0939c3a7089d63ef81d58ccc0a2611a1d374a39717d6893ea10d61ca0e87d5be7c80b29b2ed991c4a62e12d10c7f6b1b9d7ae": ["𠜎", "中文"]
    },
    "Uint8Array": {
      "f7688a104326d36c1940f6d28d746c0661d383e0d14fe8a04649444777610f5dd9565a36846ab9e9e734cf380d3a070d8ef021b5f3a50c481710a464968e3419": [new Uint8Array(0), "Hi There"]
    },
    "ArrayBuffer": {
      "f7688a104326d36c1940f6d28d746c0661d383e0d14fe8a04649444777610f5dd9565a36846ab9e9e734cf380d3a070d8ef021b5f3a50c481710a464968e3419": [new ArrayBuffer(0), "Hi There"]
    }
  },
};

const methods = ["array", "arrayBuffer", "digest", "hex"] as const;

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.sha512bits224)) {
    let i = 1;
    for (const [expected, message] of Object.entries(tests)) {
      Deno.test({
        name: `sha512/224.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new Sha512(224);
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.sha512bits256)) {
    let i = 1;
    for (const [expected, message] of Object.entries(tests)) {
      Deno.test({
        name: `sha512/256.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new Sha512(256);
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.sha512)) {
    let i = 1;
    for (const [expected, message] of Object.entries(tests)) {
      Deno.test({
        name: `sha512.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new Sha512();
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.hmacSha512bits224)) {
    let i = 1;
    for (const [expected, [key, message]] of Object.entries(tests)) {
      Deno.test({
        name: `hmacSha512/224.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new HmacSha512(key, 224);
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.hmacSha512bits256)) {
    let i = 1;
    for (const [expected, [key, message]] of Object.entries(tests)) {
      Deno.test({
        name: `hmacSha512/256.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new HmacSha512(key, 256);
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

for (const method of methods) {
  for (const [name, tests] of Object.entries(fixtures.hmacSha512)) {
    let i = 1;
    for (const [expected, [key, message]] of Object.entries(tests)) {
      Deno.test({
        name: `hmacSha512.${method}() - ${name} - #${i++}`,
        fn() {
          const algorithm = new HmacSha512(key);
          algorithm.update(message);
          const actual = method === "hex"
            ? algorithm[method]()
            : toHexString(algorithm[method]());
          assertEquals(actual, expected);
        },
      });
    }
  }
}

Deno.test("[hash/sha512] test Uint8Array from Reader", async () => {
  const data = await Deno.readFile(join(testdataDir, "hashtest"));
  const hash = new Sha512().update(data).hex();
  assertEquals(
    hash,
    "ee26b0dd4af7e749aa1a8ee3c10ae9923f618980772e473f8819a5d4940e0db27ac185f8a0e1d5f84f88bc887fd67b143732c304cc5fa9ad8e6f57f50028a8ff",
  );
});
