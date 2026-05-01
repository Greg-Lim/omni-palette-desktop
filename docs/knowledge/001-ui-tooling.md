---
title: UI Tooling
status: active
tags: [ui, egui]
---

# UI Tooling

## Summary

Omni Palette uses `egui` and `eframe` for the Windows command palette UI.

## Current Understanding

`egui` was chosen because it gives the fastest iteration loop for this project.
That matters more than maximum native look and feel while the palette behavior,
settings UI, and extension flows are still changing.

`iced` was the second-best option because its architecture is a good fit for a
state-driven app, but it was not chosen for the first implementation.

## Follow-Up

Revisit this only if `egui` becomes a clear blocker for accessibility, native
integration, performance, or long-term UI maintenance.
