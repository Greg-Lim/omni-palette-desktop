# Long-Running Stability Checklist

Use this checklist when investigating reports that Omni Palette becomes unstable
after long uptime.

## Goal

Separate true memory leaks from:

- hidden-window repaint churn
- plugin worker buildup or timeouts
- repeated extension reload stress
- external GPU / window-manager instability

## Manual Checks

### 1. Idle Hidden Test

- Start the app and leave the palette hidden for 1-2 hours.
- Confirm CPU stays near idle.
- Watch debug logs for:
  - `Runtime telemetry`
  - rising memory usage
  - rising thread count
  - repeated plugin timeouts

### 2. Open / Close Stress

- Open and close the palette repeatedly for several minutes.
- Confirm:
  - the app still responds instantly
  - memory does not climb steadily
  - thread count stays roughly flat

### 3. Plugin Stress

- Repeatedly execute WASM/plugin commands.
- Confirm:
  - commands still complete after many runs
  - timeout counts stay at zero for healthy plugins
  - thread count does not increase per execution

### 4. Reload Stress

- Edit bundled extensions during development.
- Run `Omni Palette: Reload extensions` many times.
- Confirm:
  - reload failures do not replace the last good registry
  - memory and thread count remain stable

## Debug Signals

In debug builds, watch for the periodic `Runtime telemetry` log line.

Useful fields:

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

- Hidden idle mode does not keep the UI busy unnecessarily.
- Plugin executions do not create an ever-growing number of worker threads.
- Long-running sessions stay responsive without steady resource growth.
