# Deno Documentation Parser

As a modern TypeScript run-time, Deno aims to have an online documentation website
which enables JavaScript community to see their code documentation by providing URL
of the source code. (like godoc)

This subdirectory at Deno is to maintain such functionality.

# Current plan
By using TypeScript API, we can parse a file to its syntax tree and then extract
the useful information from there, but dealing directly with AST and trying to
convert it to HTML pages might complicate things. Therefore we have to separate
the parser and the renderer.

## Step 1) Parse TS/JS AST
In this step, we try to parse an AST into the simplest possible object that contains
all the useful data which we will use to render the HTML pages.
The result of this step is an array of objects, where each of those must have a
`type` field.  
In most cases, `type` corresponds to the `kind` value from the parsed AST.

## Step 2) Render HTML pages
By using the collected data in _step 1_, we can now, produce an HTML page that we
would show to the users (developers), to do so we need to write a function for
each of the `type`s (described in _step 1_), all of these function must return an
HTML string.

### 2.1) Render code preview
First of all, we need to generate code preview for each top-level doc entity,
and we deal with syntax-highlighting and jump-to-definition in this step.

### 2.2) Render entire page
We should create a single HTML file containing documentation.

# Known issues
Current parser is a bit slow, and there are many ways to optimize it.
- Don't visit things that we don't need.  
  (like object's properties when it's not exported)
- Remove the second iteration.
- Decrease usage of arrays

