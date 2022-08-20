"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.AlgorithmTypes = exports.JwtTokenSignatureMismatched = exports.JwtTokenExpired = exports.JwtTokenNotBefore = exports.JwtTokenInvalid = exports.JwtAlorithmNotImplemented = exports.JwtAlgorithmNotImplemented = void 0;
class JwtAlgorithmNotImplemented extends Error {
    constructor(token) {
        super(`invalid JWT token: ${token}`);
        this.name = 'JwtAlgorithmNotImplemented';
    }
}
exports.JwtAlgorithmNotImplemented = JwtAlgorithmNotImplemented;
/**
 * Export for backward compatibility
 * @deprecated Use JwtAlgorithmNotImplemented instead
**/
exports.JwtAlorithmNotImplemented = JwtAlgorithmNotImplemented;
class JwtTokenInvalid extends Error {
    constructor(token) {
        super(`invalid JWT token: ${token}`);
        this.name = 'JwtTokenInvalid';
    }
}
exports.JwtTokenInvalid = JwtTokenInvalid;
class JwtTokenNotBefore extends Error {
    constructor(token) {
        super(`token (${token}) is being used before it's valid`);
        this.name = 'JwtTokenNotBefore';
    }
}
exports.JwtTokenNotBefore = JwtTokenNotBefore;
class JwtTokenExpired extends Error {
    constructor(token) {
        super(`token (${token}) expired`);
        this.name = 'JwtTokenExpired';
    }
}
exports.JwtTokenExpired = JwtTokenExpired;
class JwtTokenSignatureMismatched extends Error {
    constructor(token) {
        super(`token(${token}) signature mismatched`);
        this.name = 'JwtTokenSignatureMismatched';
    }
}
exports.JwtTokenSignatureMismatched = JwtTokenSignatureMismatched;
var AlgorithmTypes;
(function (AlgorithmTypes) {
    AlgorithmTypes["HS256"] = "HS256";
    AlgorithmTypes["HS384"] = "HS384";
    AlgorithmTypes["HS512"] = "HS512";
})(AlgorithmTypes = exports.AlgorithmTypes || (exports.AlgorithmTypes = {}));
