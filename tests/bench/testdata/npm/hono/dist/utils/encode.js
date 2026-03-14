"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.arrayBufferToBase64URL = exports.arrayBufferToBase64 = exports.utf8ToUint8Array = exports.decodeBase64URL = exports.encodeBase64URL = exports.decodeBase64 = exports.encodeBase64 = void 0;
const encodeBase64 = (str) => {
    if (str === null) {
        throw new TypeError('1st argument of "encodeBase64" should not be null.');
    }
    try {
        const encoder = new TextEncoder();
        const bytes = encoder.encode(str);
        return btoa(String.fromCharCode(...bytes));
    }
    catch { }
    try {
        return Buffer.from(str).toString('base64');
    }
    catch (e) {
        console.error('If you want to do "encodeBase64", polyfill "buffer" module.');
        throw e;
    }
};
exports.encodeBase64 = encodeBase64;
const decodeBase64 = (str) => {
    if (str === null) {
        throw new TypeError('1st argument of "decodeBase64" should not be null.');
    }
    try {
        const text = atob(str);
        const bytes = new Uint8Array(text.split('').map((c) => c.charCodeAt(0)));
        const decoder = new TextDecoder();
        return decoder.decode(bytes);
    }
    catch { }
    try {
        return Buffer.from(str, 'base64').toString();
    }
    catch (e) {
        console.error('If you want to do "decodeBase64", polyfill "buffer" module.');
        throw e;
    }
};
exports.decodeBase64 = decodeBase64;
const encodeBase64URL = (str) => {
    return (0, exports.encodeBase64)(str).replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
};
exports.encodeBase64URL = encodeBase64URL;
const decodeBase64URL = (str) => {
    const pad = (s) => {
        const diff = s.length % 4;
        if (diff === 2) {
            return `${s}==`;
        }
        if (diff === 3) {
            return `${s}=`;
        }
        return s;
    };
    return (0, exports.decodeBase64)(pad(str).replace(/-/g, '+').replace('_', '/'));
};
exports.decodeBase64URL = decodeBase64URL;
const utf8ToUint8Array = (str) => {
    const encoder = new TextEncoder();
    return encoder.encode(str);
};
exports.utf8ToUint8Array = utf8ToUint8Array;
const arrayBufferToBase64 = async (buf) => {
    if (typeof btoa === 'function') {
        return btoa(String.fromCharCode(...new Uint8Array(buf)));
    }
    try {
        return Buffer.from(String.fromCharCode(...new Uint8Array(buf))).toString('base64');
    }
    catch (e) { }
    return '';
};
exports.arrayBufferToBase64 = arrayBufferToBase64;
const arrayBufferToBase64URL = async (buf) => {
    return (await (0, exports.arrayBufferToBase64)(buf)).replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
};
exports.arrayBufferToBase64URL = arrayBufferToBase64URL;
