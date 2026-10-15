# Source migration

Target repository: `dcl-regenesislabs/godot-web-preview`.
Package name remains `@dcl-regenesislabs/mobile-preview-runtime`.

The repository starts with the existing build, tests and packaging from
`kuruk-mm/test-godot-web` at `2ebd610797d3cf366dc242d239ab25e1de19d2bc`.
Each canonical baseline was imported with `git subtree add --squash`, then the
existing browser changes were applied as a separate commit. This keeps the
upstream merge base distinct from browser modifications without importing the
complete historical commit graph into the published repository.

| Source | Canonical baseline | Tested browser source | Preserved tree |
| --- | --- | --- | --- |
| Engine | `bda49b71bf46b2362f38dccd7fdfacbaa1593b70` | `kuruk-mm/dclgodotengine@7a064b22c414e0804dedef66cec3004d9b4ae7a1` | `45d5869de947cf98bc7cb5338a9c6091b6ca7386` |
| Explorer | `cada2d93da933ffe832a31ae370308a0bc1a6712` | `kuruk-mm/godot-explorer-test@039a1c0d61121b13c7df75ceceb10feee3f46a62` | `0b2b07ea61dcc08d06d3f89a787ec3fe2328d485` |

Both imported source trees matched the tested originals exactly. Their licenses,
copyright notices, native files and upstream workflows were preserved. Only root
build/release configuration, documentation and update tooling changed during the
migration. The previous checkout and installed editor runtime remain available.

## First remote setup

After the owner authorizes publishing the source:

1. Create an empty `dcl-regenesislabs/godot-web-preview` repository with the intended
   visibility; do not initialize a competing README or license. The local `origin`
   already points at this target.
2. Publish the prepared history as `main`. Configure branch protection and require
   the build, browser and package checks. Allow merge commits for subtree update
   PRs; do not require linear history. Run the complete Mobile preview workflow
   from the new repository before the first release.
3. Allow Actions to create PRs for the daily updater. Protect the `production`
   environment with release reviewers.
4. Bootstrap the npm package if needed and configure its trusted publisher with
   organization `dcl-regenesislabs`, repository `godot-web-preview`, workflow
   `preview.yml`, environment `production`. See the runtime README.
5. Publish a matching version tag only after validation and release approval, then
   refresh the consuming editor's registry lockfile.

The migration itself creates no GitHub repository, push, npm release or scheduled
remote run. Source tree equality and local script/package tests do not substitute
for the first complete build in the new GitHub repository.

## Local validation

- The two source tree IDs above match the originals exactly.
- All 41 runtime/tooling tests and all 327 engine algorithm checks pass. Updater
  tests use real local Git repositories, including fresh clones, conflict recovery,
  engine pairing and version-change guards.
- Workflow lint, shell syntax and offline Cargo metadata checks pass.
- Packaging the previously tested export passes integrity checks and preserves all
  runtime asset hashes. A complete fresh engine/Rust build has not been rerun here.
- A read-only upstream check found explorer `5085dd729029c565f074f0c4679d5f642cd16be3`.
  Its engine pin is already covered by our tested baseline. That source update is
  intentionally separate from this migration and has not been imported.
