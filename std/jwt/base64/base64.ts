function convertBase64ToUint8Array(data: string): Uint8Array {
  const binString = atob(data);
  const size = binString.length;
  const bytes = new Uint8Array(size);
  for (let i = 0; i < size; i++) {
    bytes[i] = binString.charCodeAt(i);
  }
  return bytes;
}

// credit: https://gist.github.com/enepomnyaschih/72c423f727d395eeaa09697058238727
function convertUint8ArrayToBase64(bytes: Uint8Array): string {
  const base64abc = (() => {
    const abc = [],
      A = "A".charCodeAt(0),
      a = "a".charCodeAt(0),
      n = "0".charCodeAt(0);
    for (let i = 0; i < 26; ++i) {
      abc.push(String.fromCharCode(A + i));
    }
    for (let i = 0; i < 26; ++i) {
      abc.push(String.fromCharCode(a + i));
    }
    for (let i = 0; i < 10; ++i) {
      abc.push(String.fromCharCode(n + i));
    }
    abc.push("+");
    abc.push("/");
    return abc;
  })();

  let result = "",
    i,
    l = bytes.length;
  for (i = 2; i < l; i += 3) {
    result += base64abc[bytes[i - 2] >> 2];
    result += base64abc[((bytes[i - 2] & 0x03) << 4) | (bytes[i - 1] >> 4)];
    result += base64abc[((bytes[i - 1] & 0x0f) << 2) | (bytes[i] >> 6)];
    result += base64abc[bytes[i] & 0x3f];
  }
  if (i === l + 1) {
    // 1 octet missing
    result += base64abc[bytes[i - 2] >> 2];
    result += base64abc[(bytes[i - 2] & 0x03) << 4];
    result += "==";
  }
  if (i === l) {
    // 2 octets missing
    result += base64abc[bytes[i - 2] >> 2];
    result += base64abc[((bytes[i - 2] & 0x03) << 4) | (bytes[i - 1] >> 4)];
    result += base64abc[(bytes[i - 1] & 0x0f) << 2];
    result += "=";
  }
  return result;
}

// ucs-2 string to base64 encoded ascii
function convertStringToBase64(str: string): string {
  return btoa(unescape(encodeURIComponent(str)));
}

// base64 encoded ascii to ucs-2 string
function convertBase64ToString(str: string): string {
  return decodeURIComponent(escape(atob(str)));
}

export {
  convertBase64ToUint8Array,
  convertUint8ArrayToBase64,
  convertStringToBase64,
  convertBase64ToString,
};
