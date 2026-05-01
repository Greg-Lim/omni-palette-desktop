# Omni Palette Knowledge

This folder is the compact project memory for Omni Palette.

Each file is a knowledge note, regardless of whether it began as an ADR, design
document, checklist, or research note. Use a three-digit prefix and a short
kebab-case topic name:

```text
001-ui-tooling.md
002-extension-system.md
```

Use this front matter:

```yaml
---
title: Short Title
status: active
tags: [tag-one, tag-two]
---
```

Recommended statuses:

- `active`: current project direction or useful reference.
- `proposed`: likely direction, not implemented yet.
- `reference`: operational knowledge or checklist.
- `abandoned`: explored but not worth pursuing.

Keep notes short. Preserve the decision, current understanding, important
constraints, and useful test ideas. Remove duplicate reasoning, stale drafts,
and implementation detail that can be rediscovered from code.

## Index

- [001 UI Tooling](001-ui-tooling.md)
- [002 Extension System](002-extension-system.md)
- [003 Fuzzy Search](003-fuzzy-search.md)
- [004 Command Overrides And Focus Mode](004-command-overrides-and-focus-mode.md)
- [005 Runtime Stability](005-runtime-stability.md)
- [006 Hotkey And Accelerator Research](006-hotkey-and-accelerator-research.md)
