/** Http request built on client side */
export interface ClientRequest {
  /** Request url. ex) https://deno.land/std?query=1 */
  url: string;
  method: string;
  headers?: Headers;
  body?: string | Uint8Array | Deno.Reader;
  trailers?: () => Promise<Headers> | Headers;
}

/** Http response received on client side */
export interface ClientResponse {
  proto: string;
  status: number;
  statusText: string;
  headers: Headers;
  body: Deno.Reader;
  finalize(): Promise<void>;
}
