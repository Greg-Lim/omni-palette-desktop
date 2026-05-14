import { mount } from "svelte";
import { getCurrentWindow } from "@tauri-apps/api/window";

import App from "./App.svelte";
import Guide from "./Guide.svelte";
import "./styles.css";

const target = document.getElementById("root");

if (!target) {
  throw new Error("Missing root element");
}

const Component = getCurrentWindow().label === "guide" ? Guide : App;

export default mount(Component, { target });
