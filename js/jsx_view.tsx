import { h, Component } from "./jsx.ts";

export const Layout: Component<{ title: string }> = ({ title, children }) => (
  <html>
    <head>
      <title>{title}</title>
    </head>
    <body>
      {children}
    </body>
  </html>
)
