# os

Module provide platform-independent interface to operating system functionality.

## Usage

### userHomeDir

Returns the current user's home directory. On Unix, including macOS, it returns the \$HOME environment variable. On Windows, it returns %USERPROFILE%.
Needs permissions to access env (--allow-env).

```ts
import { userHomeDir } from "https://deno.land/std/os/mod.ts";

userHomeDir();
```
