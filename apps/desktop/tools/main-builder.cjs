// electron-builder.yml plus Electron's checksums from the installed `electron` package (tools/electron-download.cjs).
const { readFileSync } = require('node:fs')
const { join } = require('node:path')
const { parse } = require('yaml')
const root = join(__dirname, '..')
const base = parse(readFileSync(join(root, 'electron-builder.yml'), 'utf8'))
// A fork's CI builds publish to, and update from, the fork's own releases (`owner/repo`), not the upstream's.
const [owner, repo] = (process.env.GITHUB_REPOSITORY ?? '').split('/')
const publish = owner && repo ? { ...base.publish, owner, repo } : base.publish
module.exports = { ...base, publish, electronDownload: require('./electron-download.cjs') }
