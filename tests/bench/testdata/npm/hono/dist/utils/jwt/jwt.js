"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.decode = exports.verify = exports.sign = void 0;
const encode_1 = require("../../utils/encode");
const types_1 = require("./types");
const types_2 = require("./types");
var CryptoKeyFormat;
(function (CryptoKeyFormat) {
    CryptoKeyFormat["RAW"] = "raw";
    CryptoKeyFormat["PKCS8"] = "pkcs8";
    CryptoKeyFormat["SPKI"] = "spki";
    CryptoKeyFormat["JWK"] = "jwk";
})(CryptoKeyFormat || (CryptoKeyFormat = {}));
var CryptoKeyUsage;
(function (CryptoKeyUsage) {
    CryptoKeyUsage["Ecrypt"] = "encrypt";
    CryptoKeyUsage["Decrypt"] = "decrypt";
    CryptoKeyUsage["Sign"] = "sign";
    CryptoKeyUsage["Verify"] = "verify";
    CryptoKeyUsage["Deriverkey"] = "deriveKey";
    CryptoKeyUsage["DeriveBits"] = "deriveBits";
    CryptoKeyUsage["WrapKey"] = "wrapKey";
    CryptoKeyUsage["UnwrapKey"] = "unwrapKey";
})(CryptoKeyUsage || (CryptoKeyUsage = {}));
const param = (name) => {
    switch (name.toUpperCase()) {
        case 'HS256':
            return {
                name: 'HMAC',
                hash: {
                    name: 'SHA-256',
                },
            };
        case 'HS384':
            return {
                name: 'HMAC',
                hash: {
                    name: 'SHA-384',
                },
            };
        case 'HS512':
            return {
                name: 'HMAC',
                hash: {
                    name: 'SHA-512',
                },
            };
        default:
            throw new types_2.JwtAlgorithmNotImplemented(name);
    }
};
const signing = async (data, secret, alg = types_1.AlgorithmTypes.HS256) => {
    if (!crypto.subtle || !crypto.subtle.importKey) {
        throw new Error('`crypto.subtle.importKey` is undefined. JWT auth middleware requires it.');
    }
    const cryptoKey = await crypto.subtle.importKey(CryptoKeyFormat.RAW, (0, encode_1.utf8ToUint8Array)(secret), param(alg), false, [CryptoKeyUsage.Sign]);
    return await crypto.subtle.sign(param(alg), cryptoKey, (0, encode_1.utf8ToUint8Array)(data));
};
const sign = async (payload, secret, alg = types_1.AlgorithmTypes.HS256) => {
    const encodedPayload = await (0, encode_1.encodeBase64URL)(JSON.stringify(payload));
    const encodedHeader = await (0, encode_1.encodeBase64URL)(JSON.stringify({ alg, typ: 'JWT' }));
    const partialToken = `${encodedHeader}.${encodedPayload}`;
    const signature = await (0, encode_1.arrayBufferToBase64URL)(await signing(partialToken, secret, alg));
    return `${partialToken}.${signature}`;
};
exports.sign = sign;
const verify = async (token, secret, alg = types_1.AlgorithmTypes.HS256) => {
    const tokenParts = token.split('.');
    if (tokenParts.length !== 3) {
        throw new types_2.JwtTokenInvalid(token);
    }
    const { payload } = (0, exports.decode)(token);
    if (payload.nbf && payload.nbf > Math.floor(Date.now() / 1000)) {
        throw new types_2.JwtTokenNotBefore(token);
    }
    if (payload.exp && payload.exp <= Math.floor(Date.now() / 1000)) {
        throw new types_2.JwtTokenExpired(token);
    }
    const signature = await (0, encode_1.arrayBufferToBase64URL)(await signing(tokenParts.slice(0, 2).join('.'), secret, alg));
    if (signature !== tokenParts[2]) {
        throw new types_2.JwtTokenSignatureMismatched(token);
    }
    return true;
};
exports.verify = verify;
// eslint-disable-next-line
const decode = (token) => {
    try {
        const [h, p] = token.split('.');
        const header = JSON.parse((0, encode_1.decodeBase64URL)(h));
        const payload = JSON.parse((0, encode_1.decodeBase64URL)(p));
        return {
            header,
            payload,
        };
    }
    catch (e) {
        throw new types_2.JwtTokenInvalid(token);
    }
};
exports.decode = decode;
