# Touchpoints

Every file outside `overlay/` that differs from `upstream/master`, and why.
This file is the merge-cost ledger of the fork: each entry is a place that can
conflict on a sync. Keep it short and keep it honest.

Regenerate the real list with `overlay/scripts/overlay-diff.sh`; it fails if a
changed upstream file has no entry here.

Template for new entries:

```
### `path/to/file`

- What: one line describing the edit.
- Why here: why it could not be done from overlay/, hooks, or config.
- Retire when: the upstream change that would let us delete this.
```

---
