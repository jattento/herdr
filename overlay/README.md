# Overlay

This is a personal fork of `herdrdev/herdr`. Everything we own lives in this
directory; upstream never writes here, so it never conflicts on a sync.

## Layout

```
overlay/
  herdr-spaces/     <- Rust crate: Codex-style spaces (name + emoji + folders)
  scripts/          <- sync-upstream.sh, overlay-diff.sh
  TOUCHPOINTS.md    <- ledger of every edit made inside upstream's tree
```

## Rules (same as the grok-build fork)

- Keep the delta small and boring. Prefer config, then extension surfaces,
  then an overlay crate with a minimal call site, then (last) real edits in
  upstream files.
- One focused commit per customization, message prefixed `overlay:`.
- Branches: `master` carries our delta on top of upstream; `upstream` mirrors
  `herdrdev/herdr@master` exactly. Sync with `overlay/scripts/sync-upstream.sh`,
  publish with `git push --force-with-lease`.
- Every touched upstream file needs an entry in `TOUCHPOINTS.md`;
  `overlay/scripts/overlay-diff.sh` enforces it.
