import { args, exit, run } from "deno";

function qrun(args: string[]): run {
    run({
        args
    })
}

const clangFormat = () => {
    console.log('clang_format')
}

const gnFormat = () => {
    console.log('gn Format')
}

const yapf = () => {
    console.log('yapf')
}

const prettier = () => {
    console.log('prettier')
}

const rustfmt = () => {
    console.log('rustfmt')
}
