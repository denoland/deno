// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace globalThis {
  declare namespace __bootstrap {
    declare var fetchUtil: {
      requiredArguments(name: string, length: number, required: number): void;
    };

    declare var domIterable: {
      DomIterableMixin(base: any, dataSymbol: symbol): any;
    };

    declare namespace headers {
      class Headers {
      }
      type HeaderList = [string, string][];
      function headersFromHeaderList(
        list: HeaderList,
        guard:
          | "immutable"
          | "request"
          | "request-no-cors"
          | "response"
          | "none",
      ): Headers;
      function headerListFromHeaders(headers: Headers): HeaderList;
      function fillHeaders(headers: Headers, object: HeadersInit): void;
      function getDecodeSplitHeader(
        list: HeaderList,
        name: string,
      ): string[] | null;
      function guardFromHeaders(
        headers: Headers,
      ): "immutable" | "request" | "request-no-cors" | "response" | "none";
    }

    declare namespace formData {
      declare type FormData = typeof FormData;
      declare function formDataToBlob(
        formData: globalThis.FormData,
      ): Blob;
      declare function parseFormData(
        body: Uint8Array,
        boundary: string | undefined,
      ): FormData;
      declare function formDataFromEntries(entries: FormDataEntry[]): FormData;
    }

    declare namespace fetchBody {
      function mixinBody(
        prototype: any,
        bodySymbol: symbol,
        mimeTypeSymbol: symbol,
      ): void;
      class InnerBody {
        constructor(stream?: ReadableStream<Uint8Array>);
        stream: ReadableStream<Uint8Array>;
        source: null | Uint8Array | Blob | FormData;
        length: null | number;
        unusable(): boolean;
        consume(): Promise<Uint8Array>;
        clone(): InnerBody;
      }
      function extractBody(object: BodyInit): {
        body: InnerBody;
        contentType: string | null;
      };
    }

    declare namespace fetch {
      function toInnerRequest(request: Request): InnerRequest;
      function fromInnerRequest(
        inner: InnerRequest,
        signal: AbortSignal | null,
        guard:
          | "request"
          | "immutable"
          | "request-no-cors"
          | "response"
          | "none",
      ): Request;
      function redirectStatus(status: number): boolean;
      function nullBodyStatus(status: number): boolean;
      function newInnerRequest(
        method: string,
        url: any,
        headerList?: [string, string][],
        body?: globalThis.__bootstrap.fetchBody.InnerBody,
      ): InnerResponse;
      function toInnerResponse(response: Response): InnerResponse;
      function fromInnerResponse(
        inner: InnerResponse,
        guard:
          | "request"
          | "immutable"
          | "request-no-cors"
          | "response"
          | "none",
      ): Response;
      function networkError(error: string): InnerResponse;
    }
  }
}
