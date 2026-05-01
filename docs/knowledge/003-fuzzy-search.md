---
title: Fuzzy Search
status: proposed
tags: [search, ranking, ui]
---

# Fuzzy Search

## Summary

Replace simple contains-style filtering with a VS Code-inspired fuzzy scorer
that can match ordered query characters, rank by match quality and metadata, and
return match ranges for UI highlighting.

## Current Understanding

Useful behavior:

- `nt` should match `Chrome: New tab`.
- `devtools`, `dt`, or `odt` should find `Chrome: Open Developer Tools`.
- Empty query should keep default ordering and avoid highlighting everything.
- Highlighting should appear in result rows, not inside the search input.

Sorting should use hard buckets before soft score:

1. Starred commands before unstarred commands.
2. Priority bucket: `High`, then `Normal`, then `Suppressed`.
3. Fuzzy score plus bounded context and tag bonuses.
4. Prefix matches, compact spans, shorter labels, then registry order.

Within the same star bucket, a `Suppressed` command must not sort above a
`Normal` command because of a better fuzzy score. A starred suppressed command
can still sort above an unstarred normal command because the star bucket wins
first.

## Design Notes

The scorer should return a richer result than only score:

```rust
pub struct MatchResult {
    pub score: i32,
    pub ranges: Vec<MatchRange>,
}
```

The UI can render contiguous match ranges with `egui` layout jobs. Keep
keyboard navigation and Enter execution behavior unchanged.

Command metadata should eventually include priority, focus state, starred
state, tags, and original registry order. Shortcut text search, recent/frequent
ranking, and quoted exact search can come later.

## Testing

Useful coverage:

- Ordered fuzzy matches succeed.
- Wrong-order matches fail or score poorly.
- Prefix and separator matches beat scattered matches.
- Camel-case boundaries score well.
- Ranges merge consecutive indices.
- Non-ASCII labels do not panic range handling.
- Star and priority buckets sort deterministically.
- Tag matches help ranking without crowding the UI.

## Follow-Up

Implement in phases: core scorer, UI highlighting, priority sorting, stars and
tags, then query polish.
