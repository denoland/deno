


Deno.serve({ port: 4321, hostname: "::" }, (req: Response) => {
    return new Response("Hello, World")
})