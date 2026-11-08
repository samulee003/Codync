// electron-builder.yml plus Electron's checksums from the installed `electron` package (tools/electron-download.cjs).
const { readFileSync } = require('node:fs')
const { join } = require('node:path')
const { parse } = require('yaml')
const root = join(__dirname, '..')
const base = parse(readFileSync(join(root, 'electron-builder.yml'), 'utf8'))
module.exports = { ...base, electronDownload: require('./electron-download.cjs') }
