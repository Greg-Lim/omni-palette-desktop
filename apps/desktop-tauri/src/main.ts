import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";

import App from "./App.svelte";
import DebugOverlay from "./DebugOverlay.svelte";
import Guide from "./Guide.svelte";
import Settings from "./Settings.svelte";
import "./styles.css";

const target = document.getElementById("root");

if (!target) {
  throw new Error("Missing root element");
}

const label = getCurrentWindow().label;
const Component =
  label === "guide"
    ? Guide
    : label === "settings"
      ? Settings
      : label === "debug"
        ? DebugOverlay
        : App;

export default mount(Component, { target });
