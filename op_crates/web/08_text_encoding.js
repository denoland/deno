// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// The following code is based off of text-encoding at:
// https://github.com/inexorabletash/text-encoding
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.

((window) => {
  const core = Deno.core;

  const CONTINUE = null;
  const END_OF_STREAM = -1;
  const FINISHED = -1;

  function decoderError(fatal) {
    if (fatal) {
      throw new TypeError("Decoder error.");
    }
    return 0xfffd; // default code point
  }

  function inRange(a, min, max) {
    return min <= a && a <= max;
  }

  function isASCIIByte(a) {
    return inRange(a, 0x00, 0x7f);
  }

  function stringToCodePoints(input) {
    const u = [];
    for (const c of input) {
      u.push(c.codePointAt(0));
    }
    return u;
  }

  class UTF8Encoder {
    handler(codePoint) {
      if (codePoint === END_OF_STREAM) {
        return "finished";
      }

      if (inRange(codePoint, 0x00, 0x7f)) {
        return [codePoint];
      }

      let count;
      let offset;
      if (inRange(codePoint, 0x0080, 0x07ff)) {
        count = 1;
        offset = 0xc0;
      } else if (inRange(codePoint, 0x0800, 0xffff)) {
        count = 2;
        offset = 0xe0;
      } else if (inRange(codePoint, 0x10000, 0x10ffff)) {
        count = 3;
        offset = 0xf0;
      } else {
        throw TypeError(
          `Code point out of range: \\x${codePoint.toString(16)}`,
        );
      }

      const bytes = [(codePoint >> (6 * count)) + offset];

      while (count > 0) {
        const temp = codePoint >> (6 * (count - 1));
        bytes.push(0x80 | (temp & 0x3f));
        count--;
      }

      return bytes;
    }
  }

  function atob(s) {
    s = String(s);
    s = s.replace(/[\t\n\f\r ]/g, "");

    if (s.length % 4 === 0) {
      s = s.replace(/==?$/, "");
    }

    const rem = s.length % 4;
    if (rem === 1 || /[^+/0-9A-Za-z]/.test(s)) {
      throw new DOMException(
        "The string to be decoded is not correctly encoded",
        "DataDecodeError",
      );
    }

    // base64-js requires length exactly times of 4
    if (rem > 0) {
      s = s.padEnd(s.length + (4 - rem), "=");
    }

    const byteArray = base64.toByteArray(s);
    let result = "";
    for (let i = 0; i < byteArray.length; i++) {
      result += String.fromCharCode(byteArray[i]);
    }
    return result;
  }

  function btoa(s) {
    const byteArray = [];
    for (let i = 0; i < s.length; i++) {
      const charCode = s[i].charCodeAt(0);
      if (charCode > 0xff) {
        throw new TypeError(
          "The string to be encoded contains characters " +
            "outside of the Latin1 range.",
        );
      }
      byteArray.push(charCode);
    }
    const result = base64.fromByteArray(Uint8Array.from(byteArray));
    return result;
  }

  class SingleByteDecoder {
    #index = [];
    #fatal = false;

    constructor(index, { ignoreBOM = false, fatal = false } = {}) {
      if (ignoreBOM) {
        throw new TypeError("Ignoring the BOM is available only with utf-8.");
      }
      this.#fatal = fatal;
      this.#index = index;
    }
    handler(_stream, byte) {
      if (byte === END_OF_STREAM) {
        return FINISHED;
      }
      if (isASCIIByte(byte)) {
        return byte;
      }
      const codePoint = this.#index[byte - 0x80];

      if (codePoint == null) {
        return decoderError(this.#fatal);
      }

      return codePoint;
    }
  }

  // The encodingMap is a hash of labels that are indexed by the conical
  // encoding.
  const encodingMap = {
    "windows-1252": [
      "ansi_x3.4-1968",
      "ascii",
      "cp1252",
      "cp819",
      "csisolatin1",
      "ibm819",
      "iso-8859-1",
      "iso-ir-100",
      "iso8859-1",
      "iso88591",
      "iso_8859-1",
      "iso_8859-1:1987",
      "l1",
      "latin1",
      "us-ascii",
      "windows-1252",
      "x-cp1252",
    ],
    "utf-8": ["unicode-1-1-utf-8", "utf-8", "utf8"],
    ibm866: ["866", "cp866", "csibm866", "ibm866"],
    "iso-8859-2": [
      "csisolatin2",
      "iso-8859-2",
      "iso-ir-101",
      "iso8859-2",
      "iso88592",
      "iso_8859-2",
      "iso_8859-2:1987",
      "l2",
      "latin2",
    ],
    "iso-8859-3": [
      "csisolatin3",
      "iso-8859-3",
      "iso-ir-109",
      "iso8859-3",
      "iso88593",
      "iso_8859-3",
      "iso_8859-3:1988",
      "l3",
      "latin3",
    ],
    "iso-8859-4": [
      "csisolatin4",
      "iso-8859-4",
      "iso-ir-110",
      "iso8859-4",
      "iso88594",
      "iso_8859-4",
      "iso_8859-4:1988",
      "l4",
      "latin4",
    ],
    "iso-8859-5": [
      "csisolatincyrillic",
      "cyrillic",
      "iso-8859-5",
      "iso-ir-144",
      "iso8859-5",
      "iso88595",
      "iso_8859-5",
      "iso_8859-5:1988",
    ],
    "iso-8859-6": [
      "arabic",
      "asmo-708",
      "csiso88596e",
      "csiso88596i",
      "csisolatinarabic",
      "ecma-114",
      "iso-8859-6",
      "iso-8859-6-e",
      "iso-8859-6-i",
      "iso-ir-127",
      "iso8859-6",
      "iso88596",
      "iso_8859-6",
      "iso_8859-6:1987",
    ],
    "iso-8859-7": [
      "csisolatingreek",
      "ecma-118",
      "elot_928",
      "greek",
      "greek8",
      "iso-8859-7",
      "iso-ir-126",
      "iso8859-7",
      "iso88597",
      "iso_8859-7",
      "iso_8859-7:1987",
      "sun_eu_greek",
    ],
    "iso-8859-8": [
      "csiso88598e",
      "csisolatinhebrew",
      "hebrew",
      "iso-8859-8",
      "iso-8859-8-e",
      "iso-ir-138",
      "iso8859-8",
      "iso88598",
      "iso_8859-8",
      "iso_8859-8:1988",
      "visual",
    ],
    "iso-8859-10": [
      "csisolatin6",
      "iso-8859-10",
      "iso-ir-157",
      "iso8859-10",
      "iso885910",
      "l6",
      "latin6",
    ],
    "iso-8859-13": ["iso-8859-13", "iso8859-13", "iso885913"],
    "iso-8859-14": ["iso-8859-14", "iso8859-14", "iso885914"],
    "iso-8859-15": [
      "csisolatin9",
      "iso-8859-15",
      "iso8859-15",
      "iso885915",
      "iso_8859-15",
      "l9",
    ],
    "iso-8859-16": ["iso-8859-16"],
    gbk: [
      "chinese",
      "csgb2312",
      "csiso58gb231280",
      "gb2312",
      "gb_2312",
      "gb_2312-80",
      "gbk",
      "iso-ir-58",
      "x-gbk",
    ],
    gb18030: ["gb18030"],
    big5: ["big5", "big5-hkscs", "cn-big5", "csbig5", "x-x-big5"],
    "koi8-r": ["cskoi8r", "koi", "koi8", "koi8-r", "koi8_r"],
    "koi8-u": ["koi8-ru", "koi8-u"],
    macintosh: ["csmacintosh", "mac", "macintosh", "x-mac-roman"],
    "windows-874": [
      "dos-874",
      "iso-8859-11",
      "iso8859-11",
      "iso885911",
      "tis-620",
      "windows-874",
    ],
    "windows-1250": ["cp1250", "windows-1250", "x-cp1250"],
    "windows-1251": ["cp1251", "windows-1251", "x-cp1251"],
    "windows-1253": ["cp1253", "windows-1253", "x-cp1253"],
    "windows-1254": [
      "cp1254",
      "csisolatin5",
      "iso-8859-9",
      "iso-ir-148",
      "iso8859-9",
      "iso88599",
      "iso_8859-9",
      "iso_8859-9:1989",
      "l5",
      "latin5",
      "windows-1254",
      "x-cp1254",
    ],
    "windows-1255": ["cp1255", "windows-1255", "x-cp1255"],
    "windows-1256": ["cp1256", "windows-1256", "x-cp1256"],
    "windows-1257": ["cp1257", "windows-1257", "x-cp1257"],
    "windows-1258": ["cp1258", "windows-1258", "x-cp1258"],
    "x-mac-cyrillic": ["x-mac-cyrillic", "x-mac-ukrainian"],
  };
  // We convert these into a Map where every label resolves to its canonical
  // encoding type.
  const encodings = new Map();
  for (const key of Object.keys(encodingMap)) {
    const labels = encodingMap[key];
    for (const label of labels) {
      encodings.set(label, key);
    }
  }

  // A map of functions that return new instances of a decoder indexed by the
  // encoding type.
  const decoders = new Map();

  // Single byte decoders are an array of code point lookups
  const encodingIndexes = new Map();
  // deno-fmt-ignore
  encodingIndexes.set("windows-1252", [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225, 710,
    8240, 352, 8249, 338, 141, 381, 143, 144,
    8216, 8217, 8220, 8221, 8226, 8211, 8212, 732,
    8482, 353, 8250, 339, 157, 382, 376, 160,
    161, 162, 163, 164, 165, 166, 167, 168,
    169, 170, 171, 172, 173, 174, 175, 176,
    177, 178, 179, 180, 181, 182, 183, 184,
    185, 186, 187, 188, 189, 190, 191, 192,
    193, 194, 195, 196, 197, 198, 199, 200,
    201, 202, 203, 204, 205, 206, 207, 208,
    209, 210, 211, 212, 213, 214, 215, 216,
    217, 218, 219, 220, 221, 222, 223, 224,
    225, 226, 227, 228, 229, 230, 231, 232,
    233, 234, 235, 236, 237, 238, 239, 240,
    241, 242, 243, 244, 245, 246, 247, 248,
    249, 250, 251, 252, 253, 254, 255,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("ibm866", [
    1040, 1041, 1042, 1043, 1044, 1045, 1046, 1047,
    1048, 1049, 1050, 1051, 1052, 1053, 1054, 1055,
    1056, 1057, 1058, 1059, 1060, 1061, 1062, 1063,
    1064, 1065, 1066, 1067, 1068, 1069, 1070, 1071,
    1072, 1073, 1074, 1075, 1076, 1077, 1078, 1079,
    1080, 1081, 1082, 1083, 1084, 1085, 1086, 1087,
    9617, 9618, 9619, 9474, 9508, 9569, 9570, 9558,
    9557, 9571, 9553, 9559, 9565, 9564, 9563, 9488,
    9492, 9524, 9516, 9500, 9472, 9532, 9566, 9567,
    9562, 9556, 9577, 9574, 9568, 9552, 9580, 9575,
    9576, 9572, 9573, 9561, 9560, 9554, 9555, 9579,
    9578, 9496, 9484, 9608, 9604, 9612, 9616, 9600,
    1088, 1089, 1090, 1091, 1092, 1093, 1094, 1095,
    1096, 1097, 1098, 1099, 1100, 1101, 1102, 1103,
    1025, 1105, 1028, 1108, 1031, 1111, 1038, 1118,
    176, 8729, 183, 8730, 8470, 164, 9632, 160,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-2", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 260, 728, 321, 164, 317, 346, 167,
    168, 352, 350, 356, 377, 173, 381, 379,
    176, 261, 731, 322, 180, 318, 347, 711,
    184, 353, 351, 357, 378, 733, 382, 380,
    340, 193, 194, 258, 196, 313, 262, 199,
    268, 201, 280, 203, 282, 205, 206, 270,
    272, 323, 327, 211, 212, 336, 214, 215,
    344, 366, 218, 368, 220, 221, 354, 223,
    341, 225, 226, 259, 228, 314, 263, 231,
    269, 233, 281, 235, 283, 237, 238, 271,
    273, 324, 328, 243, 244, 337, 246, 247,
    345, 367, 250, 369, 252, 253, 355, 729,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-3", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 294, 728, 163, 164, null, 292, 167,
    168, 304, 350, 286, 308, 173, null, 379,
    176, 295, 178, 179, 180, 181, 293, 183,
    184, 305, 351, 287, 309, 189, null, 380,
    192, 193, 194, null, 196, 266, 264, 199,
    200, 201, 202, 203, 204, 205, 206, 207,
    null, 209, 210, 211, 212, 288, 214, 215,
    284, 217, 218, 219, 220, 364, 348, 223,
    224, 225, 226, null, 228, 267, 265, 231,
    232, 233, 234, 235, 236, 237, 238, 239,
    null, 241, 242, 243, 244, 289, 246, 247,
    285, 249, 250, 251, 252, 365, 349, 729,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-4", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 260, 312, 342, 164, 296, 315, 167,
    168, 352, 274, 290, 358, 173, 381, 175,
    176, 261, 731, 343, 180, 297, 316, 711,
    184, 353, 275, 291, 359, 330, 382, 331,
    256, 193, 194, 195, 196, 197, 198, 302,
    268, 201, 280, 203, 278, 205, 206, 298,
    272, 325, 332, 310, 212, 213, 214, 215,
    216, 370, 218, 219, 220, 360, 362, 223,
    257, 225, 226, 227, 228, 229, 230, 303,
    269, 233, 281, 235, 279, 237, 238, 299,
    273, 326, 333, 311, 244, 245, 246, 247,
    248, 371, 250, 251, 252, 361, 363, 729,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-5", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 1025, 1026, 1027, 1028, 1029, 1030, 1031,
    1032, 1033, 1034, 1035, 1036, 173, 1038, 1039,
    1040, 1041, 1042, 1043, 1044, 1045, 1046, 1047,
    1048, 1049, 1050, 1051, 1052, 1053, 1054, 1055,
    1056, 1057, 1058, 1059, 1060, 1061, 1062, 1063,
    1064, 1065, 1066, 1067, 1068, 1069, 1070, 1071,
    1072, 1073, 1074, 1075, 1076, 1077, 1078, 1079,
    1080, 1081, 1082, 1083, 1084, 1085, 1086, 1087,
    1088, 1089, 1090, 1091, 1092, 1093, 1094, 1095,
    1096, 1097, 1098, 1099, 1100, 1101, 1102, 1103,
    8470, 1105, 1106, 1107, 1108, 1109, 1110, 1111,
    1112, 1113, 1114, 1115, 1116, 167, 1118, 1119,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-6", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, null, null, null, 164, null, null, null,
    null, null, null, null, 1548, 173, null, null,
    null, null, null, null, null, null, null, null,
    null, null, null, 1563, null, null, null, 1567,
    null, 1569, 1570, 1571, 1572, 1573, 1574, 1575,
    1576, 1577, 1578, 1579, 1580, 1581, 1582, 1583,
    1584, 1585, 1586, 1587, 1588, 1589, 1590, 1591,
    1592, 1593, 1594, null, null, null, null, null,
    1600, 1601, 1602, 1603, 1604, 1605, 1606, 1607,
    1608, 1609, 1610, 1611, 1612, 1613, 1614, 1615,
    1616, 1617, 1618, null, null, null, null, null,
    null, null, null, null, null, null, null, null,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-7", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 8216, 8217, 163, 8364, 8367, 166, 167,
    168, 169, 890, 171, 172, 173, null, 8213,
    176, 177, 178, 179, 900, 901, 902, 183,
    904, 905, 906, 187, 908, 189, 910, 911,
    912, 913, 914, 915, 916, 917, 918, 919,
    920, 921, 922, 923, 924, 925, 926, 927,
    928, 929, null, 931, 932, 933, 934, 935,
    936, 937, 938, 939, 940, 941, 942, 943,
    944, 945, 946, 947, 948, 949, 950, 951,
    952, 953, 954, 955, 956, 957, 958, 959,
    960, 961, 962, 963, 964, 965, 966, 967,
    968, 969, 970, 971, 972, 973, 974, null,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-8", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, null, 162, 163, 164, 165, 166, 167,
    168, 169, 215, 171, 172, 173, 174, 175,
    176, 177, 178, 179, 180, 181, 182, 183,
    184, 185, 247, 187, 188, 189, 190, null,
    null, null, null, null, null, null, null, null,
    null, null, null, null, null, null, null, null,
    null, null, null, null, null, null, null, null,
    null, null, null, null, null, null, null, 8215,
    1488, 1489, 1490, 1491, 1492, 1493, 1494, 1495,
    1496, 1497, 1498, 1499, 1500, 1501, 1502, 1503,
    1504, 1505, 1506, 1507, 1508, 1509, 1510, 1511,
    1512, 1513, 1514, null, null, 8206, 8207, null,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-10", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 260, 274, 290, 298, 296, 310, 167,
    315, 272, 352, 358, 381, 173, 362, 330,
    176, 261, 275, 291, 299, 297, 311, 183,
    316, 273, 353, 359, 382, 8213, 363, 331,
    256, 193, 194, 195, 196, 197, 198, 302,
    268, 201, 280, 203, 278, 205, 206, 207,
    208, 325, 332, 211, 212, 213, 214, 360,
    216, 370, 218, 219, 220, 221, 222, 223,
    257, 225, 226, 227, 228, 229, 230, 303,
    269, 233, 281, 235, 279, 237, 238, 239,
    240, 326, 333, 243, 244, 245, 246, 361,
    248, 371, 250, 251, 252, 253, 254, 312,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-13", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 8221, 162, 163, 164, 8222, 166, 167,
    216, 169, 342, 171, 172, 173, 174, 198,
    176, 177, 178, 179, 8220, 181, 182, 183,
    248, 185, 343, 187, 188, 189, 190, 230,
    260, 302, 256, 262, 196, 197, 280, 274,
    268, 201, 377, 278, 290, 310, 298, 315,
    352, 323, 325, 211, 332, 213, 214, 215,
    370, 321, 346, 362, 220, 379, 381, 223,
    261, 303, 257, 263, 228, 229, 281, 275,
    269, 233, 378, 279, 291, 311, 299, 316,
    353, 324, 326, 243, 333, 245, 246, 247,
    371, 322, 347, 363, 252, 380, 382, 8217,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-14", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 7682, 7683, 163, 266, 267, 7690, 167,
    7808, 169, 7810, 7691, 7922, 173, 174, 376,
    7710, 7711, 288, 289, 7744, 7745, 182, 7766,
    7809, 7767, 7811, 7776, 7923, 7812, 7813, 7777,
    192, 193, 194, 195, 196, 197, 198, 199,
    200, 201, 202, 203, 204, 205, 206, 207,
    372, 209, 210, 211, 212, 213, 214, 7786,
    216, 217, 218, 219, 220, 221, 374, 223,
    224, 225, 226, 227, 228, 229, 230, 231,
    232, 233, 234, 235, 236, 237, 238, 239,
    373, 241, 242, 243, 244, 245, 246, 7787,
    248, 249, 250, 251, 252, 253, 375, 255,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-15", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 161, 162, 163, 8364, 165, 352, 167,
    353, 169, 170, 171, 172, 173, 174, 175,
    176, 177, 178, 179, 381, 181, 182, 183,
    382, 185, 186, 187, 338, 339, 376, 191,
    192, 193, 194, 195, 196, 197, 198, 199,
    200, 201, 202, 203, 204, 205, 206, 207,
    208, 209, 210, 211, 212, 213, 214, 215,
    216, 217, 218, 219, 220, 221, 222, 223,
    224, 225, 226, 227, 228, 229, 230, 231,
    232, 233, 234, 235, 236, 237, 238, 239,
    240, 241, 242, 243, 244, 245, 246, 247,
    248, 249, 250, 251, 252, 253, 254, 255,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("iso-8859-16", [
    128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 145, 146, 147, 148, 149, 150, 151,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 260, 261, 321, 8364, 8222, 352, 167,
    353, 169, 536, 171, 377, 173, 378, 379,
    176, 177, 268, 322, 381, 8221, 182, 183,
    382, 269, 537, 187, 338, 339, 376, 380,
    192, 193, 194, 258, 196, 262, 198, 199,
    200, 201, 202, 203, 204, 205, 206, 207,
    272, 323, 210, 211, 212, 336, 214, 346,
    368, 217, 218, 219, 220, 280, 538, 223,
    224, 225, 226, 259, 228, 263, 230, 231,
    232, 233, 234, 235, 236, 237, 238, 239,
    273, 324, 242, 243, 244, 337, 246, 347,
    369, 249, 250, 251, 252, 281, 539, 255,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("koi8-r", [
    9472, 9474, 9484, 9488, 9492, 9496, 9500, 9508,
    9516, 9524, 9532, 9600, 9604, 9608, 9612, 9616,
    9617, 9618, 9619, 8992, 9632, 8729, 8730, 8776,
    8804, 8805, 160, 8993, 176, 178, 183, 247,
    9552, 9553, 9554, 1105, 9555, 9556, 9557, 9558,
    9559, 9560, 9561, 9562, 9563, 9564, 9565, 9566,
    9567, 9568, 9569, 1025, 9570, 9571, 9572, 9573,
    9574, 9575, 9576, 9577, 9578, 9579, 9580, 169,
    1102, 1072, 1073, 1094, 1076, 1077, 1092, 1075,
    1093, 1080, 1081, 1082, 1083, 1084, 1085, 1086,
    1087, 1103, 1088, 1089, 1090, 1091, 1078, 1074,
    1100, 1099, 1079, 1096, 1101, 1097, 1095, 1098,
    1070, 1040, 1041, 1062, 1044, 1045, 1060, 1043,
    1061, 1048, 1049, 1050, 1051, 1052, 1053, 1054,
    1055, 1071, 1056, 1057, 1058, 1059, 1046, 1042,
    1068, 1067, 1047, 1064, 1069, 1065, 1063, 1066,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("koi8-u", [
    9472, 9474, 9484, 9488, 9492, 9496, 9500, 9508,
    9516, 9524, 9532, 9600, 9604, 9608, 9612, 9616,
    9617, 9618, 9619, 8992, 9632, 8729, 8730, 8776,
    8804, 8805, 160, 8993, 176, 178, 183, 247,
    9552, 9553, 9554, 1105, 1108, 9556, 1110, 1111,
    9559, 9560, 9561, 9562, 9563, 1169, 1118, 9566,
    9567, 9568, 9569, 1025, 1028, 9571, 1030, 1031,
    9574, 9575, 9576, 9577, 9578, 1168, 1038, 169,
    1102, 1072, 1073, 1094, 1076, 1077, 1092, 1075,
    1093, 1080, 1081, 1082, 1083, 1084, 1085, 1086,
    1087, 1103, 1088, 1089, 1090, 1091, 1078, 1074,
    1100, 1099, 1079, 1096, 1101, 1097, 1095, 1098,
    1070, 1040, 1041, 1062, 1044, 1045, 1060, 1043,
    1061, 1048, 1049, 1050, 1051, 1052, 1053, 1054,
    1055, 1071, 1056, 1057, 1058, 1059, 1046, 1042,
    1068, 1067, 1047, 1064, 1069, 1065, 1063, 1066,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("macintosh", [
    196, 197, 199, 201, 209, 214, 220, 225,
    224, 226, 228, 227, 229, 231, 233, 232,
    234, 235, 237, 236, 238, 239, 241, 243,
    242, 244, 246, 245, 250, 249, 251, 252,
    8224, 176, 162, 163, 167, 8226, 182, 223,
    174, 169, 8482, 180, 168, 8800, 198, 216,
    8734, 177, 8804, 8805, 165, 181, 8706, 8721,
    8719, 960, 8747, 170, 186, 937, 230, 248,
    191, 161, 172, 8730, 402, 8776, 8710, 171,
    187, 8230, 160, 192, 195, 213, 338, 339,
    8211, 8212, 8220, 8221, 8216, 8217, 247, 9674,
    255, 376, 8260, 8364, 8249, 8250, 64257, 64258,
    8225, 183, 8218, 8222, 8240, 194, 202, 193,
    203, 200, 205, 206, 207, 204, 211, 212,
    63743, 210, 218, 219, 217, 305, 710, 732,
    175, 728, 729, 730, 184, 733, 731, 711,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-874", [
    8364, 129, 130, 131, 132, 8230, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143,
    144, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    152, 153, 154, 155, 156, 157, 158, 159,
    160, 3585, 3586, 3587, 3588, 3589, 3590, 3591,
    3592, 3593, 3594, 3595, 3596, 3597, 3598, 3599,
    3600, 3601, 3602, 3603, 3604, 3605, 3606, 3607,
    3608, 3609, 3610, 3611, 3612, 3613, 3614, 3615,
    3616, 3617, 3618, 3619, 3620, 3621, 3622, 3623,
    3624, 3625, 3626, 3627, 3628, 3629, 3630, 3631,
    3632, 3633, 3634, 3635, 3636, 3637, 3638, 3639,
    3640, 3641, 3642, null, null, null, null, 3647,
    3648, 3649, 3650, 3651, 3652, 3653, 3654, 3655,
    3656, 3657, 3658, 3659, 3660, 3661, 3662, 3663,
    3664, 3665, 3666, 3667, 3668, 3669, 3670, 3671,
    3672, 3673, 3674, 3675, null, null, null, null,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1250", [
    8364, 129, 8218, 131, 8222, 8230, 8224, 8225,
    136, 8240, 352, 8249, 346, 356, 381, 377,
    144, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    152, 8482, 353, 8250, 347, 357, 382, 378,
    160, 711, 728, 321, 164, 260, 166, 167,
    168, 169, 350, 171, 172, 173, 174, 379,
    176, 177, 731, 322, 180, 181, 182, 183,
    184, 261, 351, 187, 317, 733, 318, 380,
    340, 193, 194, 258, 196, 313, 262, 199,
    268, 201, 280, 203, 282, 205, 206, 270,
    272, 323, 327, 211, 212, 336, 214, 215,
    344, 366, 218, 368, 220, 221, 354, 223,
    341, 225, 226, 259, 228, 314, 263, 231,
    269, 233, 281, 235, 283, 237, 238, 271,
    273, 324, 328, 243, 244, 337, 246, 247,
    345, 367, 250, 369, 252, 253, 355, 729,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1251", [
    1026, 1027, 8218, 1107, 8222, 8230, 8224, 8225,
    8364, 8240, 1033, 8249, 1034, 1036, 1035, 1039,
    1106, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    152, 8482, 1113, 8250, 1114, 1116, 1115, 1119,
    160, 1038, 1118, 1032, 164, 1168, 166, 167,
    1025, 169, 1028, 171, 172, 173, 174, 1031,
    176, 177, 1030, 1110, 1169, 181, 182, 183,
    1105, 8470, 1108, 187, 1112, 1029, 1109, 1111,
    1040, 1041, 1042, 1043, 1044, 1045, 1046, 1047,
    1048, 1049, 1050, 1051, 1052, 1053, 1054, 1055,
    1056, 1057, 1058, 1059, 1060, 1061, 1062, 1063,
    1064, 1065, 1066, 1067, 1068, 1069, 1070, 1071,
    1072, 1073, 1074, 1075, 1076, 1077, 1078, 1079,
    1080, 1081, 1082, 1083, 1084, 1085, 1086, 1087,
    1088, 1089, 1090, 1091, 1092, 1093, 1094, 1095,
    1096, 1097, 1098, 1099, 1100, 1101, 1102, 1103,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1253", [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225,
    136, 8240, 138, 8249, 140, 141, 142, 143,
    144, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    152, 8482, 154, 8250, 156, 157, 158, 159,
    160, 901, 902, 163, 164, 165, 166, 167,
    168, 169, null, 171, 172, 173, 174, 8213,
    176, 177, 178, 179, 900, 181, 182, 183,
    904, 905, 906, 187, 908, 189, 910, 911,
    912, 913, 914, 915, 916, 917, 918, 919,
    920, 921, 922, 923, 924, 925, 926, 927,
    928, 929, null, 931, 932, 933, 934, 935,
    936, 937, 938, 939, 940, 941, 942, 943,
    944, 945, 946, 947, 948, 949, 950, 951,
    952, 953, 954, 955, 956, 957, 958, 959,
    960, 961, 962, 963, 964, 965, 966, 967,
    968, 969, 970, 971, 972, 973, 974, null,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1254", [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225,
    710, 8240, 352, 8249, 338, 141, 142, 143,
    144, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    732, 8482, 353, 8250, 339, 157, 158, 376,
    160, 161, 162, 163, 164, 165, 166, 167,
    168, 169, 170, 171, 172, 173, 174, 175,
    176, 177, 178, 179, 180, 181, 182, 183,
    184, 185, 186, 187, 188, 189, 190, 191,
    192, 193, 194, 195, 196, 197, 198, 199,
    200, 201, 202, 203, 204, 205, 206, 207,
    286, 209, 210, 211, 212, 213, 214, 215,
    216, 217, 218, 219, 220, 304, 350, 223,
    224, 225, 226, 227, 228, 229, 230, 231,
    232, 233, 234, 235, 236, 237, 238, 239,
    287, 241, 242, 243, 244, 245, 246, 247,
    248, 249, 250, 251, 252, 305, 351, 255,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1255", [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225,
    710, 8240, 138, 8249, 140, 141, 142, 143,
    144, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    732, 8482, 154, 8250, 156, 157, 158, 159,
    160, 161, 162, 163, 8362, 165, 166, 167,
    168, 169, 215, 171, 172, 173, 174, 175,
    176, 177, 178, 179, 180, 181, 182, 183,
    184, 185, 247, 187, 188, 189, 190, 191,
    1456, 1457, 1458, 1459, 1460, 1461, 1462, 1463,
    1464, 1465, 1466, 1467, 1468, 1469, 1470, 1471,
    1472, 1473, 1474, 1475, 1520, 1521, 1522, 1523,
    1524, null, null, null, null, null, null, null,
    1488, 1489, 1490, 1491, 1492, 1493, 1494, 1495,
    1496, 1497, 1498, 1499, 1500, 1501, 1502, 1503,
    1504, 1505, 1506, 1507, 1508, 1509, 1510, 1511,
    1512, 1513, 1514, null, null, 8206, 8207, null,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1256", [
    8364, 1662, 8218, 402, 8222, 8230, 8224, 8225,
    710, 8240, 1657, 8249, 338, 1670, 1688, 1672,
    1711, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    1705, 8482, 1681, 8250, 339, 8204, 8205, 1722,
    160, 1548, 162, 163, 164, 165, 166, 167,
    168, 169, 1726, 171, 172, 173, 174, 175,
    176, 177, 178, 179, 180, 181, 182, 183,
    184, 185, 1563, 187, 188, 189, 190, 1567,
    1729, 1569, 1570, 1571, 1572, 1573, 1574, 1575,
    1576, 1577, 1578, 1579, 1580, 1581, 1582, 1583,
    1584, 1585, 1586, 1587, 1588, 1589, 1590, 215,
    1591, 1592, 1593, 1594, 1600, 1601, 1602, 1603,
    224, 1604, 226, 1605, 1606, 1607, 1608, 231,
    232, 233, 234, 235, 1609, 1610, 238, 239,
    1611, 1612, 1613, 1614, 244, 1615, 1616, 247,
    1617, 249, 1618, 251, 252, 8206, 8207, 1746,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1257", [
    8364, 129, 8218, 131, 8222, 8230, 8224, 8225,
    136, 8240, 138, 8249, 140, 168, 711, 184,
    144, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    152, 8482, 154, 8250, 156, 175, 731, 159,
    160, null, 162, 163, 164, null, 166, 167,
    216, 169, 342, 171, 172, 173, 174, 198,
    176, 177, 178, 179, 180, 181, 182, 183,
    248, 185, 343, 187, 188, 189, 190, 230,
    260, 302, 256, 262, 196, 197, 280, 274,
    268, 201, 377, 278, 290, 310, 298, 315,
    352, 323, 325, 211, 332, 213, 214, 215,
    370, 321, 346, 362, 220, 379, 381, 223,
    261, 303, 257, 263, 228, 229, 281, 275,
    269, 233, 378, 279, 291, 311, 299, 316,
    353, 324, 326, 243, 333, 245, 246, 247,
    371, 322, 347, 363, 252, 380, 382, 729,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("windows-1258", [
    8364, 129, 8218, 402, 8222, 8230, 8224, 8225,
    710, 8240, 138, 8249, 338, 141, 142, 143,
    144, 8216, 8217, 8220, 8221, 8226, 8211, 8212,
    732, 8482, 154, 8250, 339, 157, 158, 376,
    160, 161, 162, 163, 164, 165, 166, 167,
    168, 169, 170, 171, 172, 173, 174, 175,
    176, 177, 178, 179, 180, 181, 182, 183,
    184, 185, 186, 187, 188, 189, 190, 191,
    192, 193, 194, 258, 196, 197, 198, 199,
    200, 201, 202, 203, 768, 205, 206, 207,
    272, 209, 777, 211, 212, 416, 214, 215,
    216, 217, 218, 219, 220, 431, 771, 223,
    224, 225, 226, 259, 228, 229, 230, 231,
    232, 233, 234, 235, 769, 237, 238, 239,
    273, 241, 803, 243, 244, 417, 246, 247,
    248, 249, 250, 251, 252, 432, 8363, 255,
  ]);

  // deno-fmt-ignore
  encodingIndexes.set("x-mac-cyrillic", [
    1040, 1041, 1042, 1043, 1044, 1045, 1046, 1047,
    1048, 1049, 1050, 1051, 1052, 1053, 1054, 1055,
    1056, 1057, 1058, 1059, 1060, 1061, 1062, 1063,
    1064, 1065, 1066, 1067, 1068, 1069, 1070, 1071,
    8224, 176, 1168, 163, 167, 8226, 182, 1030,
    174, 169, 8482, 1026, 1106, 8800, 1027, 1107,
    8734, 177, 8804, 8805, 1110, 181, 1169, 1032,
    1028, 1108, 1031, 1111, 1033, 1113, 1034, 1114,
    1112, 1029, 172, 8730, 402, 8776, 8710, 171,
    187, 8230, 160, 1035, 1115, 1036, 1116, 1109,
    8211, 8212, 8220, 8221, 8216, 8217, 247, 8222,
    1038, 1118, 1039, 1119, 8470, 1025, 1105, 1103,
    1072, 1073, 1074, 1075, 1076, 1077, 1078, 1079,
    1080, 1081, 1082, 1083, 1084, 1085, 1086, 1087,
    1088, 1089, 1090, 1091, 1092, 1093, 1094, 1095,
    1096, 1097, 1098, 1099, 1100, 1101, 1102, 8364,
  ]);

  for (const [key, index] of encodingIndexes) {
    decoders.set(key, (options) => {
      return new SingleByteDecoder(index, options);
    });
  }

  function codePointsToString(codePoints) {
    let s = "";
    for (const cp of codePoints) {
      s += String.fromCodePoint(cp);
    }
    return s;
  }

  class Stream {
    #tokens = [];
    constructor(tokens) {
      this.#tokens = [...tokens];
      this.#tokens.reverse();
    }

    endOfStream() {
      return !this.#tokens.length;
    }

    read() {
      return !this.#tokens.length ? END_OF_STREAM : this.#tokens.pop();
    }

    prepend(token) {
      if (Array.isArray(token)) {
        while (token.length) {
          this.#tokens.push(token.pop());
        }
      } else {
        this.#tokens.push(token);
      }
    }

    push(token) {
      if (Array.isArray(token)) {
        while (token.length) {
          this.#tokens.unshift(token.shift());
        }
      } else {
        this.#tokens.unshift(token);
      }
    }
  }

  function isEitherArrayBuffer(x) {
    return (
      x instanceof SharedArrayBuffer ||
      x instanceof ArrayBuffer ||
      typeof x === "undefined"
    );
  }

  class TextDecoder {
    #encoding = "";

    get encoding() {
      return this.#encoding;
    }
    fatal = false;
    ignoreBOM = false;

    constructor(label = "utf-8", options = { fatal: false }) {
      if (options.ignoreBOM) {
        this.ignoreBOM = true;
      }
      if (options.fatal) {
        this.fatal = true;
      }
      const _label = String(label).trim().toLowerCase();
      const encoding = encodings.get(_label);
      if (!encoding) {
        throw new RangeError(
          `The encoding label provided ('${label}') is invalid.`,
        );
      }
      if (!decoders.has(encoding) && encoding !== "utf-8") {
        throw new RangeError(`Internal decoder ('${encoding}') not found.`);
      }
      this.#encoding = encoding;
    }

    decode(input, options = { stream: false }) {
      if (options.stream) {
        throw new TypeError("Stream not supported.");
      }

      let bytes;
      if (input instanceof Uint8Array) {
        bytes = input;
      } else if (isEitherArrayBuffer(input)) {
        bytes = new Uint8Array(input);
      } else if (
        typeof input === "object" &&
        input !== null &&
        "buffer" in input &&
        isEitherArrayBuffer(input.buffer)
      ) {
        bytes = new Uint8Array(
          input.buffer,
          input.byteOffset,
          input.byteLength,
        );
      } else {
        throw new TypeError(
          "Provided input is not of type ArrayBuffer or ArrayBufferView",
        );
      }

      // For simple utf-8 decoding "Deno.core.decode" can be used for performance
      if (
        this.#encoding === "utf-8" &&
        this.fatal === false &&
        this.ignoreBOM === false
      ) {
        return core.decode(bytes);
      }

      // For performance reasons we utilise a highly optimised decoder instead of
      // the general decoder.
      if (this.#encoding === "utf-8") {
        return decodeUtf8(bytes, this.fatal, this.ignoreBOM);
      }

      const decoder = decoders.get(this.#encoding)({
        fatal: this.fatal,
        ignoreBOM: this.ignoreBOM,
      });
      const inputStream = new Stream(bytes);
      const output = [];

      while (true) {
        const result = decoder.handler(inputStream, inputStream.read());
        if (result === FINISHED) {
          break;
        }

        if (result !== CONTINUE) {
          output.push(result);
        }
      }

      if (output.length > 0 && output[0] === 0xfeff) {
        output.shift();
      }

      return codePointsToString(output);
    }

    get [Symbol.toStringTag]() {
      return "TextDecoder";
    }
  }

  class TextEncoder {
    encoding = "utf-8";
    encode(input = "") {
      input = String(input);
      // Deno.core.encode() provides very efficient utf-8 encoding
      if (this.encoding === "utf-8") {
        return core.encode(input);
      }

      const encoder = new UTF8Encoder();
      const inputStream = new Stream(stringToCodePoints(input));
      const output = [];

      while (true) {
        const result = encoder.handler(inputStream.read());
        if (result === "finished") {
          break;
        }
        output.push(...result);
      }

      return new Uint8Array(output);
    }
    encodeInto(input, dest) {
      const encoder = new UTF8Encoder();
      const inputStream = new Stream(stringToCodePoints(input));

      let written = 0;
      let read = 0;
      while (true) {
        const result = encoder.handler(inputStream.read());
        if (result === "finished") {
          break;
        }
        if (dest.length - written >= result.length) {
          read++;
          dest.set(result, written);
          written += result.length;
          if (result.length > 3) {
            // increment read a second time if greater than U+FFFF
            read++;
          }
        } else {
          break;
        }
      }

      return {
        read,
        written,
      };
    }
    get [Symbol.toStringTag]() {
      return "TextEncoder";
    }
  }

  // This function is based on Bjoern Hoehrmann's DFA UTF-8 decoder.
  // See http://bjoern.hoehrmann.de/utf-8/decoder/dfa/ for details.
  //
  // Copyright (c) 2008-2009 Bjoern Hoehrmann <bjoern@hoehrmann.de>
  //
  // Permission is hereby granted, free of charge, to any person obtaining a copy
  // of this software and associated documentation files (the "Software"), to deal
  // in the Software without restriction, including without limitation the rights
  // to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
  // copies of the Software, and to permit persons to whom the Software is
  // furnished to do so, subject to the following conditions:
  //
  // The above copyright notice and this permission notice shall be included in
  // all copies or substantial portions of the Software.
  //
  // THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
  // IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
  // FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
  // AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
  // LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
  // OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
  // SOFTWARE.
  function decodeUtf8(input, fatal, ignoreBOM) {
    let outString = "";

    // Prepare a buffer so that we don't have to do a lot of string concats, which
    // are very slow.
    const outBufferLength = Math.min(1024, input.length);
    const outBuffer = new Uint16Array(outBufferLength);
    let outIndex = 0;

    let state = 0;
    let codepoint = 0;
    let type;

    let i =
      ignoreBOM && input[0] === 0xef && input[1] === 0xbb && input[2] === 0xbf
        ? 3
        : 0;

    for (; i < input.length; ++i) {
      // Encoding error handling
      if (state === 12 || (state !== 0 && (input[i] & 0xc0) !== 0x80)) {
        if (fatal) {
          throw new TypeError(
            `Decoder error. Invalid byte in sequence at position ${i} in data.`,
          );
        }
        outBuffer[outIndex++] = 0xfffd; // Replacement character
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
        state = 0;
      }

      // deno-fmt-ignore
      // deno-fmt-ignore
      type = [
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,  9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
         7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
         8,8,2,2,2,2,2,2,2,2,2,2,2,2,2,2,  2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
        10,3,3,3,3,3,3,3,3,3,3,3,3,4,3,3, 11,6,6,6,5,8,8,8,8,8,8,8,8,8,8,8
      ][input[i]];
      codepoint = state !== 0
        ? (input[i] & 0x3f) | (codepoint << 6)
        : (0xff >> type) & input[i];
      // deno-fmt-ignore
      // deno-fmt-ignore
      state = [
         0,12,24,36,60,96,84,12,12,12,48,72, 12,12,12,12,12,12,12,12,12,12,12,12,
        12, 0,12,12,12,12,12, 0,12, 0,12,12, 12,24,12,12,12,12,12,24,12,24,12,12,
        12,12,12,12,12,12,12,24,12,12,12,12, 12,24,12,12,12,12,12,12,12,24,12,12,
        12,12,12,12,12,12,12,36,12,36,12,12, 12,36,12,12,12,12,12,36,12,36,12,12,
        12,36,12,12,12,12,12,12,12,12,12,12
      ][state + type];

      if (state !== 0) continue;

      // Add codepoint to buffer (as charcodes for utf-16), and flush buffer to
      // string if needed.
      if (codepoint > 0xffff) {
        outBuffer[outIndex++] = 0xd7c0 + (codepoint >> 10);
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
        outBuffer[outIndex++] = 0xdc00 | (codepoint & 0x3ff);
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
      } else {
        outBuffer[outIndex++] = codepoint;
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
      }
    }

    // Add a replacement character if we ended in the middle of a sequence or
    // encountered an invalid code at the end.
    if (state !== 0) {
      if (fatal) throw new TypeError(`Decoder error. Unexpected end of data.`);
      outBuffer[outIndex++] = 0xfffd; // Replacement character
    }

    // Final flush of buffer
    outString += String.fromCharCode.apply(
      null,
      outBuffer.subarray(0, outIndex),
    );

    return outString;
  }

  // Following code is forked from https://github.com/beatgammit/base64-js
  // Copyright (c) 2014 Jameson Little. MIT License.
  const lookup = [];
  const revLookup = [];

  const code =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  for (let i = 0, len = code.length; i < len; ++i) {
    lookup[i] = code[i];
    revLookup[code.charCodeAt(i)] = i;
  }

  // Support decoding URL-safe base64 strings, as Node.js does.
  // See: https://en.wikipedia.org/wiki/Base64#URL_applications
  revLookup["-".charCodeAt(0)] = 62;
  revLookup["_".charCodeAt(0)] = 63;

  function getLens(b64) {
    const len = b64.length;

    if (len % 4 > 0) {
      throw new Error("Invalid string. Length must be a multiple of 4");
    }

    // Trim off extra bytes after placeholder bytes are found
    // See: https://github.com/beatgammit/base64-js/issues/42
    let validLen = b64.indexOf("=");
    if (validLen === -1) validLen = len;

    const placeHoldersLen = validLen === len ? 0 : 4 - (validLen % 4);

    return [validLen, placeHoldersLen];
  }

  // base64 is 4/3 + up to two characters of the original data
  function byteLength(b64) {
    const lens = getLens(b64);
    const validLen = lens[0];
    const placeHoldersLen = lens[1];
    return ((validLen + placeHoldersLen) * 3) / 4 - placeHoldersLen;
  }

  function _byteLength(b64, validLen, placeHoldersLen) {
    return ((validLen + placeHoldersLen) * 3) / 4 - placeHoldersLen;
  }

  function toByteArray(b64) {
    let tmp;
    const lens = getLens(b64);
    const validLen = lens[0];
    const placeHoldersLen = lens[1];

    const arr = new Uint8Array(_byteLength(b64, validLen, placeHoldersLen));

    let curByte = 0;

    // if there are placeholders, only get up to the last complete 4 chars
    const len = placeHoldersLen > 0 ? validLen - 4 : validLen;

    let i;
    for (i = 0; i < len; i += 4) {
      tmp = (revLookup[b64.charCodeAt(i)] << 18) |
        (revLookup[b64.charCodeAt(i + 1)] << 12) |
        (revLookup[b64.charCodeAt(i + 2)] << 6) |
        revLookup[b64.charCodeAt(i + 3)];
      arr[curByte++] = (tmp >> 16) & 0xff;
      arr[curByte++] = (tmp >> 8) & 0xff;
      arr[curByte++] = tmp & 0xff;
    }

    if (placeHoldersLen === 2) {
      tmp = (revLookup[b64.charCodeAt(i)] << 2) |
        (revLookup[b64.charCodeAt(i + 1)] >> 4);
      arr[curByte++] = tmp & 0xff;
    }

    if (placeHoldersLen === 1) {
      tmp = (revLookup[b64.charCodeAt(i)] << 10) |
        (revLookup[b64.charCodeAt(i + 1)] << 4) |
        (revLookup[b64.charCodeAt(i + 2)] >> 2);
      arr[curByte++] = (tmp >> 8) & 0xff;
      arr[curByte++] = tmp & 0xff;
    }

    return arr;
  }

  function tripletToBase64(num) {
    return (
      lookup[(num >> 18) & 0x3f] +
      lookup[(num >> 12) & 0x3f] +
      lookup[(num >> 6) & 0x3f] +
      lookup[num & 0x3f]
    );
  }

  function encodeChunk(uint8, start, end) {
    let tmp;
    const output = [];
    for (let i = start; i < end; i += 3) {
      tmp = ((uint8[i] << 16) & 0xff0000) +
        ((uint8[i + 1] << 8) & 0xff00) +
        (uint8[i + 2] & 0xff);
      output.push(tripletToBase64(tmp));
    }
    return output.join("");
  }

  function fromByteArray(uint8) {
    let tmp;
    const len = uint8.length;
    const extraBytes = len % 3; // if we have 1 byte left, pad 2 bytes
    const parts = [];
    const maxChunkLength = 16383; // must be multiple of 3

    // go through the array every three bytes, we'll deal with trailing stuff later
    for (let i = 0, len2 = len - extraBytes; i < len2; i += maxChunkLength) {
      parts.push(
        encodeChunk(
          uint8,
          i,
          i + maxChunkLength > len2 ? len2 : i + maxChunkLength,
        ),
      );
    }

    // pad the end with zeros, but make sure to not forget the extra bytes
    if (extraBytes === 1) {
      tmp = uint8[len - 1];
      parts.push(lookup[tmp >> 2] + lookup[(tmp << 4) & 0x3f] + "==");
    } else if (extraBytes === 2) {
      tmp = (uint8[len - 2] << 8) + uint8[len - 1];
      parts.push(
        lookup[tmp >> 10] +
          lookup[(tmp >> 4) & 0x3f] +
          lookup[(tmp << 2) & 0x3f] +
          "=",
      );
    }

    return parts.join("");
  }

  const base64 = {
    byteLength,
    toByteArray,
    fromByteArray,
  };

  window.TextEncoder = TextEncoder;
  window.TextDecoder = TextDecoder;
  window.atob = atob;
  window.btoa = btoa;
  window.__bootstrap = window.__bootstrap || {};
  window.__bootstrap.base64 = base64;
})(this);
