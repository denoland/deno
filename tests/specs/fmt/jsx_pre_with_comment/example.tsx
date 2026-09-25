import { ErrorPageProps } from "$fresh/server.ts";

export default function ErrorPage500(props: ErrorPageProps) {
  return (
    <pre>
      {props.url}
      {/* {props.url} */}
    </pre>
  );
}
