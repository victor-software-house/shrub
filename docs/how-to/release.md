# How to release

Releases go through [verctl](https://github.com/victor-software-house/verctl), configured in `.ctl/ver.yaml`.

1. A pull request that changes a published crate adds a fragment, `.changeset/<slug>.md`:

   ```md
   ---
   shrub: patch
   ---

   What changed, for the changelog.
   ```

   Every crate shares the workspace version, so the fragment always names `shrub`.
2. When it merges, the Version PR workflow opens or updates the `version-packages` pull request.
   It bumps `[workspace.package].version`, rewrites the exact internal requirements in `[workspace.dependencies]`, refreshes `Cargo.lock`, and writes `CHANGELOG.md`.
3. Merging the Version PR runs the publish workflow.
   It runs `cargo publish --locked --workspace`, which uploads every publishable crate in dependency order, then tags the release and opens the GitHub Release.

Preview a release locally with `MISE_ENV=release mise x -- verctl prepare --dry-run`.
Versions stay on the `0.0.x` line; moving to `0.1.0` is a separate decision.
