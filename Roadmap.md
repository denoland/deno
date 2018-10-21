# Deno Roadmap

API and Feature requests should be submitted as PRs to this document.

## Security Model (partially implemented)

- We want to be secure by default; user should be able to run untrusted code,
  like the web.
- Threat model:
  - Modifiying/deleting local files
  - Leaking private information
- Disallowed default:
  - Network access
  - Local write access
  - Non-JS extensions
  - Subprocesses
  - Env access
- Allowed default:
  - Local read access.
  - argv, stdout, stderr, stdin access always allowed.
  - Maybe: temp dir write access. (But what if they create symlinks there?)
- The user gets prompted when the software tries to do something it doesn't have
  the privilege for.
- Have an option to get a stack trace when access is requested.
- Worried that granting access per file will give a false sense of security due
  to monkey patching techniques. Access should be granted per program (js
  context).

Example security prompts. Options are: YES, NO, PRINT STACK

```
Program requests write access to "~/.ssh/id_rsa". Grant? [yNs]
http://gist.github.com/asdfasd.js requests network access to "www.facebook.com". Grant? [yNs]
Program requests access to environment variables. Grant? [yNs]
Program requests to spawn `rm -rf /`. Grant? [yNs]
```

- cli flags to grant access ahead of time --allow-all --allow-write --allow-net
  --allow-env --allow-exec
- in version two we will add ability to give finer grain access
  --allow-net=facebook.com

## Top-level Await (Not Implemented)

[#471](https://github.com/denoland/deno/issues/471)

This will be put off until at least deno2 Milestone1 is complete. One of the
major problems is that top-level await calls are not syntactically valid
TypeScript.

### [Broken] List dependencies of a program.

Currently broken: https://github.com/denoland/deno/issues/1011

```
% deno --deps http://gist.com/blah.js
http://gist.com/blah.js
http://gist.com/dep.js
https://github.com/denoland/deno/master/testing.js
%
```
