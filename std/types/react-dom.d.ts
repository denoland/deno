// These types are adapted from
// https://github.com/DefinitelyTyped/DefinitelyTyped to work under Deno.
//
// Type definitions for React (react-dom) 16.9
// Project: http://facebook.github.io/react/
// Definitions by: Asana <https://asana.com>
//                 AssureSign <http://www.assuresign.com>
//                 Microsoft <https://microsoft.com>
//                 MartynasZilinskas <https://github.com/MartynasZilinskas>
//                 Josh Rutherford <https://github.com/theruther4d>
//                 Jessica Franco <https://github.com/Jessidhia>
// Definitions: https://github.com/DefinitelyTyped/DefinitelyTyped
// TypeScript Version: 2.8

// NOTE: Users of the `experimental` builds of React should add a reference
// to 'react-dom/experimental' in their project. See experimental.d.ts's top comment
// for reference and documentation on how exactly to do it.

/* eslint-disable */

export as namespace ReactDOM;

import {
  ReactInstance,
  Component,
  ComponentState,
  ReactElement,
  SFCElement,
  CElement,
  DOMAttributes,
  DOMElement,
  ReactNode,
  ReactPortal,
} from "./react.d.ts";

export function findDOMNode(
  instance: ReactInstance | null | undefined
): Element | null | Text;
export function unmountComponentAtNode(container: Element): boolean;

export function createPortal(
  children: ReactNode,
  container: Element,
  key?: null | string
): ReactPortal;

export const version: string;
export const render: Renderer;
export const hydrate: Renderer;

export function unstable_batchedUpdates<A, B>(
  callback: (a: A, b: B) => any,
  a: A,
  b: B
): void;
export function unstable_batchedUpdates<A>(callback: (a: A) => any, a: A): void;
export function unstable_batchedUpdates(callback: () => any): void;

export function unstable_renderSubtreeIntoContainer<T extends Element>(
  parentComponent: Component<any>,
  element: DOMElement<DOMAttributes<T>, T>,
  container: Element,
  callback?: (element: T) => any
): T;
export function unstable_renderSubtreeIntoContainer<
  P,
  T extends Component<P, ComponentState>
>(
  parentComponent: Component<any>,
  element: CElement<P, T>,
  container: Element,
  callback?: (component: T) => any
): T;
export function unstable_renderSubtreeIntoContainer<P>(
  parentComponent: Component<any>,
  element: ReactElement<P>,
  container: Element,
  callback?: (component?: Component<P, ComponentState> | Element) => any
): Component<P, ComponentState> | Element | void;

export interface Renderer {
  // Deprecated(render): The return value is deprecated.
  // In future releases the render function's return type will be void.

  <T extends Element>(
    element: DOMElement<DOMAttributes<T>, T>,
    container: Element | null,
    callback?: () => void
  ): T;

  (
    element: Array<DOMElement<DOMAttributes<any>, any>>,
    container: Element | null,
    callback?: () => void
  ): Element;

  (
    element: SFCElement<any> | Array<SFCElement<any>>,
    container: Element | null,
    callback?: () => void
  ): void;

  <P, T extends Component<P, ComponentState>>(
    element: CElement<P, T>,
    container: Element | null,
    callback?: () => void
  ): T;

  (
    element: Array<CElement<any, Component<any, ComponentState>>>,
    container: Element | null,
    callback?: () => void
  ): Component<any, ComponentState>;

  <P>(
    element: ReactElement<P>,
    container: Element | null,
    callback?: () => void
  ): Component<P, ComponentState> | Element | void;

  (element: ReactElement[], container: Element | null, callback?: () => void):
    | Component<any, ComponentState>
    | Element
    | void;
}
