import * as OctokitTypes from "/-/@octokit/types@v6.34.0-YyJmQq3Ajv0ACapsoOVh/dist=es2019,mode=types/index.d.ts";
import { RequestError } from "/-/@octokit/request-error@v2.1.0-eEdMdUdHjpMHPFvA4aCp/dist=es2019,mode=types/index.d.ts";
import { Octokit } from "./index.d.ts";
export declare type RequestParameters = OctokitTypes.RequestParameters;
export interface OctokitOptions {
    authStrategy?: any;
    auth?: any;
    userAgent?: string;
    previews?: string[];
    baseUrl?: string;
    log?: {
        debug: (message: string) => unknown;
        info: (message: string) => unknown;
        warn: (message: string) => unknown;
        error: (message: string) => unknown;
    };
    request?: OctokitTypes.RequestRequestOptions;
    timeZone?: string;
    [option: string]: any;
}
export declare type Constructor<T> = new (...args: any[]) => T;
export declare type ReturnTypeOf<T extends AnyFunction | AnyFunction[]> = T extends AnyFunction ? ReturnType<T> : T extends AnyFunction[] ? UnionToIntersection<Exclude<ReturnType<T[number]>, void>> : never;
/**
 * @author https://stackoverflow.com/users/2887218/jcalz
 * @see https://stackoverflow.com/a/50375286/10325032
 */
export declare type UnionToIntersection<Union> = (Union extends any ? (argument: Union) => void : never) extends (argument: infer Intersection) => void ? Intersection : never;
declare type AnyFunction = (...args: any) => any;
export declare type OctokitPlugin = (octokit: Octokit, options: OctokitOptions) => {
    [key: string]: any;
} | void;
export declare type Hooks = {
    request: {
        Options: Required<OctokitTypes.EndpointDefaults>;
        Result: OctokitTypes.OctokitResponse<any>;
        Error: RequestError | Error;
    };
    [key: string]: {
        Options: unknown;
        Result: unknown;
        Error: unknown;
    };
};
export {};
