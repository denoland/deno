export type Element = null | string | number | View | View[];
export type ElementFactory = string | Component;
export type PropsWithChildren<P = {}> = { children?: Element[] } & P;
export type Component<P = {}> = (
  params: PropsWithChildren<P>
) => View<Component, P>;

export type ElementAttribute = number | string | boolean | undefined | null;
export type View<T extends ElementFactory = Component, P = {}> = {
  type: T;
  props?: P;
  children?: Element[];
};

/** Create JSX View from function component. This is default JSX Factory for typescript compiler.
 *
 *       // JSX expressions are only available in TSX/JSX files.
 *       const Link: Deno.Component<{href: string, class: string}> = ({children, ...props}) => (
 *         <a {...props}>{children}</a>
 *       )
 */
export function h<P = {}>(type: Component<P>): View<Component, P>;
export function h<P = {}>(
  type: string,
  props?: P,
  ...children: Element[]
): View<string, P>;
export function h<T extends ElementFactory, P = {}>(
  type: T,
  props?: P,
  ...children: Element[]
): View<T, P> {
  return { type, props, children };
}

function isValid(x: unknown): x is ElementAttribute {
  return (
    typeof x === "string" ||
    typeof x === "number" ||
    typeof x === "boolean" ||
    x === null
  );
}

/** Render JSX Element to string.
 *
 *       // JSX expressions are only available in TSX/JSX files.
 *       Deno.renderToString(<a href="https://deno.land">Deno</a>)
 *       // In plain js/ts files, use with Deno.h.
 *       Deno.renderToString(Deno.h("a", {href: "https://deno.land"}, "Deno"))
 */
export function renderToString(node: Element): string {
  if (node === null) {
    return "";
  } else if (typeof node === "string") {
    return node;
  } else if (typeof node === "number") {
    return `${node.toString()}`;
  } else if (Array.isArray(node)) {
    return node.map(renderToString).join("");
  }
  let name: string;
  const props = node.props || {};
  const children = node.children || [];
  if (typeof node.type === "function") {
    const rendererd = node.type({ ...props, children: node.children });
    return renderToString(rendererd);
  } else if (typeof node.type === "string") {
    name = node.type;
  } else {
    throw new Error("invalid node type: " + node.type);
  }
  const propsStr = Object.entries(props)
    .filter(([_, v]) => isValid(v))
    .map(([k, v]) => {
      return `${k}="${v}"`;
    })
    .join(" ");
  const childrenStr = children.map(v => renderToString(v)).join("");
  return `<${name}${propsStr ? ` ${propsStr} ` : ""}>${childrenStr}</${name}>`;
}
