import { RequestHeaders } from "./RequestHeaders.d.ts";
import { RequestMethod } from "./RequestMethod.d.ts";
import { RequestRequestOptions } from "./RequestRequestOptions.d.ts";
import { Url } from "./Url.d.ts";
/**
 * Generic request options as they are returned by the `endpoint()` method
 */
export declare type RequestOptions = {
    method: RequestMethod;
    url: Url;
    headers: RequestHeaders;
    body?: any;
    request?: RequestRequestOptions;
};
