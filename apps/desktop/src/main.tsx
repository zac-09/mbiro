import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { App } from "./App";
import { MenuBar } from "./components/MenuBar";
import "./index.css";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");

// One bundle serves both windows; the window's label says which screen it is.
const isMenuBar = getCurrentWebviewWindow().label === "menubar";

createRoot(root).render(<StrictMode>{isMenuBar ? <MenuBar /> : <App />}</StrictMode>);
