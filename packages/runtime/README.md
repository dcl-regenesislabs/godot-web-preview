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

The `preview.yml` workflow tests and audits every PR/main change. Changes beyond
documentation also build the runtime and create the `mobile-preview-npm` artifact
(retained for 14 days). Extracting its tarball exposes the web files in
`package/dist/`; there is no duplicate raw-export artifact. All three browser
graphics regressions and package verification must pass before the run is accepted
or a release can publish. Manual runs and release tags always build.
Publishing goes through
[Decentraland's Oddish Action](https://github.com/decentraland/oddish-action), pinned
to a reviewed commit, using npm OIDC on a GitHub-hosted runner. The workflow
extracts the tested tarball, verifies its asset hashes, and gives that package
directory to Oddish, which adds the source commit to `package.json` and publishes
the same runtime assets:

- Every `main` push publishes a snapshot to the `next` dist-tag, versioned
  `<package.json version>-<run id>.commit-<sha>`. `check-dist` accepts that
  prerelease of the manifest version, so `prepack` still verifies every asset.
- A numeric tag matching `package.json` (`0.1.0`, **without `v`**) publishes that
  stable version to `latest`. Oddish treats `v0.1.0` as a snapshot; the release
  check rejects that format.

PRs, documentation-only pushes and manual validation runs never publish. Consumers
pin an exact `next` version; `npm view @dcl-regenesislabs/mobile-preview-runtime
dist-tags.next` prints the newest one.

Configure npm trusted publishing for GitHub owner `dcl-regenesislabs`, repository
`godot-web-preview`, workflow `preview.yml`, environment `production`, with direct
`npm publish` and dist-tag management allowed. Both publishes run in that environment
because the trusted publisher is bound to its name; do not give it required
reviewers, or every `next` snapshot waits for approval. The publish job uses
`id-token: write`; no npm token is stored. It pins npm 11.21.0 for OIDC dist-tag
support. Releases include npm provenance linking each package to this public source
repository and its build. See [npm's trusted publishing guide](https://docs.npmjs.com/trusted-publishers/).

The package name was bootstrapped with a manual `0.0.0-bootstrap.0` publication on
the `bootstrap` dist-tag; `latest` points at that placeholder until the first stable
tag ships, so consumers install a `next` version, not `latest`. Oddish also uploads
its own copy of the tarball as a workflow artifact with the repository's default
retention; lower that default in the repository's Actions settings if needed. After
a publish, refresh the consumer's lockfile so it records the registry tarball
integrity before merging the consumer update.

Engine and explorer sources are included as subtrees. Builds use the temporary
repository `GITHUB_TOKEN` and public toolchain downloads; neither this repository
nor a consuming editor requires `GODOT_SOURCE_TOKEN`.
