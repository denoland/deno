import commonjs from 'rollup-plugin-commonjs';
import nodeResolve from 'rollup-plugin-node-resolve';
import typescript from 'rollup-plugin-typescript2';

export default {
  output: {
    format: 'es',
    sourcemap: true
  },

  plugins: [
    nodeResolve({
      jsnext: true,
      main: true
    }),

    commonjs({
      namedExports: {
        '../../third_party/node_modules/typescript/lib/typescript.js': [ 'version' ]
      }
    }),

    typescript({
      cacheRoot: `${require('os').tmpdir()}/.rpt2_cache`,
      tsconfig: '../../tsconfig.json',
      include: [ '*.ts+(|x)', '../../**/*.ts+(|x)' ],
      exclude: [ '*.d.ts', '../../**/*.d.ts' ]
    })
  ],
  external: [
    'fs',
    'path',
    'os',
    'crypto',
    'buffer',
    'module'
  ]
}