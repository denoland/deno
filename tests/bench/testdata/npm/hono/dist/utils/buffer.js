"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.bufferToString = exports.timingSafeEqual = exports.equal = void 0;
const crypto_1 = require("./crypto");
const equal = (a, b) => {
    if (a === b) {
        return true;
    }
    if (a.byteLength !== b.byteLength) {
        return false;
    }
    const va = new DataView(a);
    const vb = new DataView(b);
    let i = va.byteLength;
    while (i--) {
        if (va.getUint8(i) !== vb.getUint8(i)) {
            return false;
        }
    }
    return true;
};
exports.equal = equal;
const timingSafeEqual = async (a, b, hashFunction) => {
    if (!hashFunction) {
        hashFunction = crypto_1.sha256;
    }
    const sa = await hashFunction(a);
    const sb = await hashFunction(b);
    return sa === sb && a === b;
};
exports.timingSafeEqual = timingSafeEqual;
const bufferToString = (buffer) => {
    if (buffer instanceof ArrayBuffer) {
        const enc = new TextDecoder('utf-8');
        return enc.decode(buffer);
    }
    return buffer;
};
exports.bufferToString = bufferToString;
