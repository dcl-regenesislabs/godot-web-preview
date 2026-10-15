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
npm tarball after all three browser graphics regressions pass. A stable `X.Y.Z`
tag matching `package.json` publishes through
[Decentraland's Oddish Action](https://github.com/decentraland/oddish-action), pinned
to a reviewed commit, using npm OIDC on a GitHub-hosted runner. The workflow
extracts the tested tarball, verifies its asset hashes, and gives that package
directory to Oddish. Oddish adds the source commit to `package.json` and publishes
the same runtime assets to `latest`.

Use numeric tags such as `0.1.0`, **without `v`**. Oddish treats `v0.1.0` as a
snapshot rather than a stable release; the release check rejects that format.
PRs, main pushes and manual validation runs never publish.

Configure npm trusted publishing for GitHub owner `dcl-regenesislabs`, repository
`godot-web-preview`, workflow `preview.yml`, environment `production`, with direct
`npm publish` and dist-tag management allowed. Oddish updates `latest` after
publishing. The publish job uses `id-token: write`; no npm token is
stored. It pins npm 11.21.0 for OIDC dist-tag support. Releases include npm provenance linking each package to this public source
repository and its build. See [npm's trusted publishing guide](https://docs.npmjs.com/trusted-publishers/).

For the first release, a maintainer must bootstrap the package name and register
the trusted publisher before tagging the first OIDC release. Reserve `0.1.0` for
CI: use a separate bootstrap prerelease and dist-tag, such as
`0.0.0-bootstrap.0` on `bootstrap`, for the initial manual publication. The package is not
currently published; consumers targeting `0.1.0` cannot install it until that
release exists. Approve the complete CI run and configure the GitHub `production`
environment's release reviewers before releasing. Publish source/runtime first,
then refresh the consumer's lockfile to record the registry tarball integrity
before merging the consumer update.

Engine and explorer sources are included as subtrees. Builds use the temporary
repository `GITHUB_TOKEN` and public toolchain downloads; neither this repository
nor a consuming editor requires `GODOT_SOURCE_TOKEN`.
