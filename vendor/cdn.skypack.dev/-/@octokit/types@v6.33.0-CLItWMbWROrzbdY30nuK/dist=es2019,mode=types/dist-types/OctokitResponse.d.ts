import { ResponseHeaders } from "./ResponseHeaders.d.ts";
import { Url } from "./Url.d.ts";
export declare type OctokitResponse<T, S extends number = number> = {
  headers: ResponseHeaders;
  /**
   * http response code
   */
  status: S;
  /**
   * URL of response after all redirects
   */
  url: Url;
  /**
   * Response data as documented in the REST API reference documentation at https://docs.github.com/rest/reference
   */
  data: T;
};
