export declare class JwtAlgorithmNotImplemented extends Error {
    constructor(token: string);
}
/**
 * Export for backward compatibility
 * @deprecated Use JwtAlgorithmNotImplemented instead
**/
export declare const JwtAlorithmNotImplemented: typeof JwtAlgorithmNotImplemented;
export declare class JwtTokenInvalid extends Error {
    constructor(token: string);
}
export declare class JwtTokenNotBefore extends Error {
    constructor(token: string);
}
export declare class JwtTokenExpired extends Error {
    constructor(token: string);
}
export declare class JwtTokenSignatureMismatched extends Error {
    constructor(token: string);
}
export declare enum AlgorithmTypes {
    HS256 = "HS256",
    HS384 = "HS384",
    HS512 = "HS512"
}
