export declare type Cookie = Record<string, string>;
export declare type CookieOptions = {
    domain?: string;
    expires?: Date;
    httpOnly?: boolean;
    maxAge?: number;
    path?: string;
    secure?: boolean;
    signed?: boolean;
    sameSite?: 'Strict' | 'Lax' | 'None';
};
export declare const parse: (cookie: string) => Cookie;
export declare const serialize: (name: string, value: string, opt?: CookieOptions) => string;
