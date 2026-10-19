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
2. When it merges, the release workflow opens or updates the Version PR from the `verctl/release/main` branch.
   It bumps `[workspace.package].version`, rewrites the exact internal requirements in `[workspace.dependencies]`, refreshes `Cargo.lock`, and writes `CHANGELOG.md`.
3. Merging the Version PR runs the release workflow again, which publishes the version commit.
   verctl uploads each publishable crate in dependency order, one `cargo publish --package` at a time, then tags the release and opens the GitHub Release.
   A crate the crates.io index already lists is skipped, so a run that stops partway resumes on the next push to `main` or a manual run of the release workflow.
   The workflow releases the version commit itself, so later merges to `main` do not block it.
   crates.io accepts 5 new crates at once, then 1 every 10 minutes; a run that hits that limit stops before the tag and names the time to retry.

Preview a release locally with `MISE_ENV=release mise x -- verctl prepare --dry-run`.
Versions stay on the `0.0.x` line; moving to `0.1.0` is a separate decision.
