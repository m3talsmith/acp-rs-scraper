# Contributing

Hooks, code layout, and tests are in [docs/USAGE.md](docs/USAGE.md#contributing).

## Releases

A GitHub Release for a version that is not yet on `main` is a **draft**. Do not publish it. The maintainer publishes the draft by hand after that version is merged into `main`.

`main` is at 2.16.0. Drafts cover 2.17.0 through 2.28.0 (including 2.24.1) and 3.0.0 through 3.3.2. Releases for 2.16.0 and earlier, once published, stay published. Do not convert them to drafts.

`.githooks/post-commit` creates an annotated git tag `vX.Y.Z` when the version in the root `Cargo.toml` changes. It does not create a GitHub Release. There is no CI workflow that publishes a release on a tag.

When a release is created:

- use `gh release create <tag> --draft --target <tag-commit> --title <tag> --notes-file <notes>`;
- take the notes from the matching `## [X.Y.Z]` section of `CHANGELOG.md` (for 2.25.0–2.28.0, that section lives on the tag, because the current changelog folds that work into 3.0.0);
- attach no files.

`--draft` is required. A release without it is published, and that is reserved for the maintainer after the merge.
