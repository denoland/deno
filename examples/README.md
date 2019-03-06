# Deno Example Programs

These files are accessible for import via "https://deno.land/std/examples/".

Try it:

```
> deno https://deno.land/std/examples/gist.ts README.md
```

## Alias Based Installation

Add this to your `.bash_profile`

```
export GIST_TOKEN=ABC # Generate at https://github.com/settings/tokens
alias gist="deno https://deno.land/std/examples/gist.ts --allow-net --allow-env"
```
