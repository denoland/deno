# Fetch Data

When building any sort of web application developers will usually need to
retrieve data from somewhere else on the web. This works no differently in Deno
than in any other JavaScript application, just call the the `fetch()` method.

The exception with Deno occurs when running a script which makes a call over the
web. Deno is secure by default which means access to IO (Input / Output) is
prohibited. To make a call over the web Deno must be explicitly told
it is ok to do so. This is achieved by adding the `--allow-net` flag to the
`deno run` command.

**Command:** `deno run --allow-net fetch.ts`

```js
async function callApi(url: string): Promise<Response> {
  try {
    return await fetch(url);
  } catch (e) {
    throw new Error(e.message);
  }
}

/**
 * Output: JSON Data
**/
callApi("https://api.github.com/users/denoland")
  .then((response) => {
    return response.json();
  })
  .then((json) => console.log(json));

/**
 * Output: HTML Data
**/
callApi("https://deno.land/")
  .then((response) => {
    return response.text();
  })
  .then((text) => console.log(text));

/**
 * Output: Error Message
**/
callApi("https://does.not.exist/")
  .catch((error) => console.log(error));
```
