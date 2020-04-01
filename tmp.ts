import { contentType } from 'https://deno.land/std/media_types/mod.ts'

for (const filename of [
  'foo.js',
  'foo.html',
  'foo.wasm',
  'foo.exe',
  'foo.zip',
  'Makefile',
  'hello',
  'sh.sh',
  '.bashrc',
  '.gitignore',
  'abc.gitignore',
  'foo.json'
]) {
  console.log(filename, '->', contentType(filename))
}
