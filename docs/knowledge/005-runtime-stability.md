---
title: Runtime Stability
status: reference
tags: [stability, diagnostics, plugins]
---

# Runtime Stability

## Summary

Use this checklist when investigating reports that Omni Palette becomes unstable
after long uptime.

## Checks

Idle hidden test:

- Start the app and leave the palette hidden for 1 to 2 hours.
- Confirm CPU stays near idle.
- Watch logs for rising memory, rising thread count, repeated plugin timeouts,
  and `Runtime telemetry`.

Open and close stress:

- Open and close the palette repeatedly for several minutes.
- Confirm responsiveness stays instant.
- Confirm memory does not climb steadily and thread count stays roughly flat.

Plugin stress:

- Repeatedly execute plugin commands.
- Confirm commands complete after many runs.
- Confirm timeout counts stay at zero for healthy plugins.
- Confirm thread count does not increase per execution.

Reload stress:

- Run `Omni Palette: Reload extensions` many times during development.
- Confirm reload failures do not replace the last good registry.
- Confirm memory and thread count remain stable.

## Debug Signals

In debug builds, watch periodic `Runtime telemetry` fields:

- `visible`
- `apps`
- `plugins`
- `plugin_started`
- `plugin_completed`
- `plugin_failed`
- `plugin_timed_out`
- `memory_private_bytes`
- `thread_count`

## Success Criteria

Hidden idle mode does not keep the UI busy, plugin executions do not create an
ever-growing number of worker threads, and long-running sessions stay responsive
without steady resource growth.
