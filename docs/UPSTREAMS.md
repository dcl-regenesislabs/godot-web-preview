# Updating the mobile sources

`godotengine/` and `godot-explorer/` are squashed Git subtrees. Their files and
browser modifications belong to this repository. `upstreams.json` records the
canonical repositories, tracked branches and imported upstream commits. The npm
manifest also records this repository's commit and the two complete source tree IDs.

The engine source follows `decentraland/godotengine` on `4.6.2`; the explorer follows
`decentraland/godot-explorer` on `main`. The updater reads the explorer's
`GODOT_BUILD_SHA` and imports that engine revision when it advances. It retains an
already tested engine baseline that is ahead of that pin instead of downgrading.
An engine version change, branch override or rewritten upstream history requires
manual review. Native importer binaries remain pinned by the explorer itself.

## Local updates

Use Node 24 and Git with `git subtree` installed:

```sh
git switch -c chore/update-mobile-upstreams
node scripts/upstream.mjs check
node scripts/upstream.mjs sync
node --test tests/upstream.test.mjs
./build.sh emsdk engine native wasm preview
```

`check` fetches upstream commits and prints the proposed updates without changing
tracked files. `sync` requires a clean feature branch, imports the same compatible
pair with subtree merges, and commits each imported revision. Neither command
pushes or publishes anything. A fresh clone works without manually adding remotes.

If a merge conflicts, the command stops with Git's conflict state intact and does
not advance that source's recorded revision. Resolve and commit the merge, then
rerun `sync` to finish recording the revision and process any remaining source.
`git merge --abort` abandons only the current conflicted merge; earlier completed
imports remain committed. Do not use `--ours` or `--theirs` across a whole subtree.

Review the resulting diff and run the gameplay, scene UI and scene-comms fixtures.
Clean merges can still change behavior. Keep the last tested npm version in the
editor until the update has merged and its `next` snapshot is published.

Merge source-update PRs with a **merge commit**. Squashing or rebasing the PR can
discard the subtree import metadata needed by the next update. The updater rejects
imports whose history no longer matches the lock file. Ordinary browser-only PRs
can still be squash-merged. The `--squash` used by `git subtree` condenses upstream
history into an import commit; that import commit must remain in this repository.

## Daily workflow

`sync-upstream.yml` runs at 09:17 UTC daily or manually from `main`. It skips while
an existing `chore/sync-upstream-*` PR is open, so it cannot overwrite a reviewer's
or agent's repairs. Otherwise it imports available updates, creates one draft PR
and explicitly dispatches `preview.yml` on that branch. No source changes means
no PR and no expensive build. Conflicts fail the sync job for investigation.

Allow GitHub Actions to create pull requests in repository settings. The sync job
uses the repository's temporary `GITHUB_TOKEN` for branch/PR creation and workflow
dispatch; it has no npm publishing permissions. Build jobs use read-only tokens,
and only the publish job (main pushes and release tags) requests OIDC credentials. Sync validation also
enables the optional native Deno compatibility check. No PAT or
`GODOT_SOURCE_TOKEN` is needed. Approve any additional PR workflow runs GitHub holds
for approval, and review the dispatched validation run before merging.

The updater uses Git and fixed scripts. An agent can resolve a failed sync or test
on the PR branch; no agent service, credentials or automatic merging is configured.
The source folders' own `.github/workflows/` are retained as upstream files and
are not active workflows in this repository.

## Other source dependencies

Emscripten remains a public submodule, initialized by `./build.sh emsdk`. The
explorer's optional iOS engine gitlink is registered in the root `.gitmodules` at
`godot-explorer/plugins/dcl-godot-ios/godot`; browser CI does not initialize it.
Ordinary clones already contain all browser engine and explorer source files.
