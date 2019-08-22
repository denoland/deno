const promiseMap = new Map();
let nextPromiseId = 1;

const opNamespace = Deno.ops.mainWorker;

if (typeof TextEncoder === "undefined") {
  TextEncoder = function TextEncoder() {};
  TextEncoder.prototype.encode = function encode(str) {
    "use strict";
    var Len = str.length,
      resPos = -1;
    // The Uint8Array's length must be at least 3x the length of the string because an invalid UTF-16
    //  takes up the equivelent space of 3 UTF-8 characters to encode it properly. However, Array's
    //  have an auto expanding length and 1.5x should be just the right balance for most uses.
    var resArr =
      typeof Uint8Array === "undefined"
        ? new Array(Len * 1.5)
        : new Uint8Array(Len * 3);
    for (var point = 0, nextcode = 0, i = 0; i !== Len; ) {
      (point = str.charCodeAt(i)), (i += 1);
      if (point >= 0xd800 && point <= 0xdbff) {
        if (i === Len) {
          resArr[(resPos += 1)] = 0xef /*0b11101111*/;
          resArr[(resPos += 1)] = 0xbf /*0b10111111*/;
          resArr[(resPos += 1)] = 0xbd /*0b10111101*/;
          break;
        }
        // https://mathiasbynens.be/notes/javascript-encoding#surrogate-formulae
        nextcode = str.charCodeAt(i);
        if (nextcode >= 0xdc00 && nextcode <= 0xdfff) {
          point = (point - 0xd800) * 0x400 + nextcode - 0xdc00 + 0x10000;
          i += 1;
          if (point > 0xffff) {
            resArr[(resPos += 1)] = (0x1e /*0b11110*/ << 3) | (point >>> 18);
            resArr[(resPos += 1)] =
              (0x2 /*0b10*/ << 6) | ((point >>> 12) & 0x3f) /*0b00111111*/;
            resArr[(resPos += 1)] =
              (0x2 /*0b10*/ << 6) | ((point >>> 6) & 0x3f) /*0b00111111*/;
            resArr[(resPos += 1)] =
              (0x2 /*0b10*/ << 6) | (point & 0x3f) /*0b00111111*/;
            continue;
          }
        } else {
          resArr[(resPos += 1)] = 0xef /*0b11101111*/;
          resArr[(resPos += 1)] = 0xbf /*0b10111111*/;
          resArr[(resPos += 1)] = 0xbd /*0b10111101*/;
          continue;
        }
      }
      if (point <= 0x007f) {
        resArr[(resPos += 1)] = (0x0 /*0b0*/ << 7) | point;
      } else if (point <= 0x07ff) {
        resArr[(resPos += 1)] = (0x6 /*0b110*/ << 5) | (point >>> 6);
        resArr[(resPos += 1)] =
          (0x2 /*0b10*/ << 6) | (point & 0x3f) /*0b00111111*/;
      } else {
        resArr[(resPos += 1)] = (0xe /*0b1110*/ << 4) | (point >>> 12);
        resArr[(resPos += 1)] =
          (0x2 /*0b10*/ << 6) | ((point >>> 6) & 0x3f) /*0b00111111*/;
        resArr[(resPos += 1)] =
          (0x2 /*0b10*/ << 6) | (point & 0x3f) /*0b00111111*/;
      }
    }
    if (typeof Uint8Array !== "undefined")
      return resArr.subarray(0, resPos + 1);
    // else // IE 6-9
    resArr.length = resPos + 1; // trim off extra weight
    return resArr;
  };
  TextEncoder.prototype.toString = function() {
    return "[object TextEncoder]";
  };
  try {
    // Object.defineProperty only works on DOM prototypes in IE8
    Object.defineProperty(TextEncoder.prototype, "encoding", {
      get: function() {
        if (TextEncoder.prototype.isPrototypeOf(this)) return "utf-8";
        else throw TypeError("Illegal invocation");
      }
    });
  } catch (e) {
    /*IE6-8 fallback*/ TextEncoder.prototype.encoding = "utf-8";
  }
  if (typeof Symbol !== "undefined")
    TextEncoder.prototype[Symbol.toStringTag] = "TextEncoder";
}

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function createResolvable() {
  let methods;
  const promise = new Promise((resolve, reject) => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods);
}

const textEncoder = new TextEncoder();

let listenOpId;
opNamespace.listen = id => {
  listenOpId = id;
};
/** Accepts a connection, returns rid. */
function listen(options) {
  Deno.core.dispatch(listenOpId, textEncoder.encode(JSON.stringify(options)));
}

const listenParams = {
  address: "127.0.0.1:4544",
  workerCount: 16,
  workerScript: `
        const requestBuf = new Uint8Array(64 * 1024);
        const responseString = "HTTP/1.1 200 OK\\r\\nContent-Length: 12\\r\\n\\r\\nHello World\\n";
        const responseBuf = new Uint8Array(
            responseString
              .split("")
              .map(c => c.charCodeAt(0))
        );

        async function serve(rid) {
            while (true) {
                const nread = await read(rid, requestBuf);
                if (nread <= 0) {
                    break;
                }
            
                const nwritten = await write(rid, responseBuf);
                if (nwritten < 0) {
                    break;
                }
            }
            close(rid);
        }

        async function work() {
            Deno.core.setAsyncHandler(handleAsyncMsgFromRust);

            Deno.core.print(responseString)

            while (true) {
                const rid = await accept();
                if (rid < 0) {
                  return;
                }
                serve(rid);
            }
        }

        work();
    `
};

listen(listenParams);
