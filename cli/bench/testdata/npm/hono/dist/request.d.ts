import type { Body } from './utils/body';
import type { Cookie } from './utils/cookie';
declare global {
    interface Request<ParamKeyType extends string = string> {
        param: {
            (key: ParamKeyType): string;
            (): Record<ParamKeyType, string>;
        };
        paramData?: Record<ParamKeyType, string>;
        query: {
            (key: string): string;
            (): Record<string, string>;
        };
        queries: {
            (key: string): string[];
            (): Record<string, string[]>;
        };
        header: {
            (name: string): string;
            (): Record<string, string>;
        };
        cookie: {
            (name: string): string;
            (): Cookie;
        };
        parsedBody?: Promise<Body>;
        parseBody: {
            (): Promise<Body>;
        };
    }
}
export declare function extendRequestPrototype(): void;
