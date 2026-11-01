# Contributing

Bug reports, fixes and new engine support are welcome. For anything larger than a fix, open an issue first so the change can be agreed on before you write it.

## Building

Follow **Build from source** in the [README](README.md). For interface work, `pnpm -C app/ui dev` runs the whole interface in a browser against an in-memory backend, without building the Rust side.

## Checks

A pull request should pass these, from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm -C app/ui check
pnpm -C app/ui build
node scripts/licenses.mjs --check
```

When you add or update a dependency, run `node scripts/licenses.mjs` to refresh the licence list shown in the app, and commit the result. New Rust dependencies are pinned to an exact version (`=x.y.z`).

## Style

Match the surrounding code. A few rules hold everywhere:

- Rust: Clippy's pedantic lints are on for the whole workspace; no `unwrap` outside tests; user values always go to the server as parameters, never spliced into SQL.
- TypeScript is strict and Svelte uses runes.
- Every visible string goes into both `app/ui/messages/en.json` and `app/ui/messages/it.json`. If you do not write Italian, put the English text in both files and say so in the pull request.
- Colours, spacing, radii and fonts come from the CSS tokens of the four styles, never from literal values in a component. Check a visual change in all four styles, light and dark.
- Small commits that each do one thing.

## Licence

NiceDB is licensed under the GNU General Public License, version 3 or later, with the additional permission described in [LICENSE](LICENSE). By contributing, you agree that your contribution is licensed the same way.
