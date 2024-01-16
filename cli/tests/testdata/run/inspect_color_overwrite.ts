console.log(
  Deno[Deno.internal].inspectArgs(["%cfoo%cbar", "", "color: red"], {
    colors: true,
  }),
);
