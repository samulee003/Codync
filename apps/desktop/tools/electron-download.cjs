// electron-builder's `electronDownload` for every build: the checksums the `electron` package (from npm) ships for its
// own release. Packaging checks the Electron and FFmpeg downloads against them, wherever they come from (a mirror set
// with ELECTRON_MIRROR included), and never downloads SHASUMS256.txt.
// `force: false` marks these as @electron/get options.
module.exports = { force: false, checksums: require('electron/checksums.json') }
