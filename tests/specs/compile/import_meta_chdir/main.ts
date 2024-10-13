import { fileURLToPath } from 'node:url'
import process from 'node:process'

const __dirname = fileURLToPath(new URL('.', import.meta.url))

process.chdir(__dirname)
