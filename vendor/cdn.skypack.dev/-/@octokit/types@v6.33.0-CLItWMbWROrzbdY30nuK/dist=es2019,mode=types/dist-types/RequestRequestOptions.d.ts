import { Fetch } from "./Fetch.d.ts";
import { Signal } from "./Signal.d.ts";
/**
 * Octokit-specific request options which are ignored for the actual request, but can be used by Octokit or plugins to manipulate how the request is sent or how a response is handled
 */
export declare type RequestRequestOptions = {
    /**
     * Node only. Useful for custom proxy, certificate, or dns lookup.
     *
     * @see https://nodejs.org/api/http.html#http_class_http_agent
     */
    agent?: unknown;
    /**
     * Custom replacement for built-in fetch method. Useful for testing or request hooks.
     */
    fetch?: Fetch;
    /**
     * Use an `AbortController` instance to cancel a request. In node you can only cancel streamed requests.
     */
    signal?: Signal;
    /**
     * Node only. Request/response timeout in ms, it resets on redirect. 0 to disable (OS limit applies). `options.request.signal` is recommended instead.
     */
    timeout?: number;
    [option: string]: any;
};
