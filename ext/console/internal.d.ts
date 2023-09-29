// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_console/01_console.js" {
  function createFilteredInspectProxy<TObject>(params: {
    object: TObject;
    keys: (keyof TObject)[];
    evaluate: boolean;
  }): Record<string, unknown>;
}

declare module "ext:deno_console/02_jsx.js" {
  import { JSXSerializeAdapter, SharedVNode } from "ext:deno_console/jsx";

  export const reactAdapter: JSXSerializeAdapter;
  export const preactAdapter: JSXSerializeAdapter;

  export function serialize(
    adapter: JSXSerializeAdapter,
    vnode: SharedVNode,
    level: number,
    limit: number
  ): string;
}

declare module "ext:deno_console/jsx" {
  export interface ComponentFunction<T = unknown> {
    (props: T): VElement;
    displayName?: string;
    contenxtType?: {
      displayName?: string;
      Provider?: (props: unknown) => VElement;
      Consumer?: (props: unknown) => VElement;
    };
  }

  export type VChild =
    | string
    | number
    | null
    | undefined
    // deno-lint-ignore ban-types
    | Function
    | SharedVNode;
  export type VElement = VChild | VElement[] | Iterable<VElement>;
  export type NormalizedChild =
    | string
    // deno-lint-ignore ban-types
    | Function
    | SharedVNode
    | NormalizedChild[];

  export interface PreactComponentInstance {
    sub?: unknown;
    displayName?: string;
  }

  export interface SharedVNode {
    $$typeof?: symbol;
    // In Preact "null" means text and "props.data" is the text.
    // In React symbols are for special nodes like Fragments
    type:
      | string
      | ComponentFunction
      | null
      | number
      | symbol
      // React
      | {
          $$typeof: symbol;
          displayName?: string;
          type: ComponentFunction;
          render?: ComponentFunction;
        };
    props: Record<string, unknown>;
    key: null | undefined | number | string;
  }

  export interface JSXSerializeAdapter {
    getTextIfTextNode(x: SharedVNode): string | null;
    isFragment(x: SharedVNode): boolean;
    getName(x: SharedVNode): string;
    isValidElement(x: unknown): x is SharedVNode;
  }
}
