# Mobile preview runtime

`@dcl-regenesislabs/mobile-preview-runtime` contains the prebuilt Godot WebGPU
engine, mobile client and scene assets. Applications install an exact npm version;
they do not need this repository or a Rust/Godot toolchain.

The package exports `./package.json`, `./manifest.json` and the executable `./check`.
Runtime files live in `dist/` beside the manifest, which records each file's size
and SHA-256 plus the repository commit, source tree IDs and imported upstream revisions. The package includes
the client, engine and bundled browser LiveKit license notices. It has no install scripts or dependencies.

After building the web preview from the workspace root:

```sh
./build.sh preview
npm run build --prefix packages/runtime
npm test --prefix packages/runtime
npm pack ./packages/runtime
```

Packing verifies the complete artifact inventory and rejects modified, missing,
unlisted or symlinked files. Generated files and tarballs are ignored by Git.
Applications provide their own scene relay, capture adapter and asset server.

The `preview.yml` workflow builds and tests every PR/main change and creates an
npm tarball after all three browser graphics regressions pass. A stable `vX.Y.Z`
tag matching `package.json` publishes that tarball to `latest` using npm OIDC on
a GitHub-hosted runner. PRs, main pushes and manual validation runs never publish.

Configure npm trusted publishing for GitHub owner `dcl-regenesislabs`, repository
`godot-web-preview`, workflow `preview.yml`, environment `production`, with direct
`npm publish` allowed. The publish job uses `id-token: write`; no npm token is
stored. Provenance is disabled to support a private source repository; enable it
if this repository is published publicly. See [npm's trusted publishing guide](https://docs.npmjs.com/trusted-publishers/).

For the first release, a maintainer must bootstrap the package name and register
the trusted publisher before tagging the first OIDC release. The package is not
currently published; consumers targeting `0.1.0` cannot install it until that
release exists. Publish source/runtime first, then refresh the consumer's lockfile
to record the registry tarball integrity before merging the consumer update.

Engine and explorer sources are included as subtrees. Builds use the temporary
repository `GITHUB_TOKEN` and public toolchain downloads; neither this repository
nor a consuming editor requires `GODOT_SOURCE_TOKEN`.
