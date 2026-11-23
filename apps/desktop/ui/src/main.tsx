import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { applyStoredTheme } from "./theme";
import "./styles.css";

applyStoredTheme();

const root = document.getElementById("root");
if (root === null) {
  throw new Error("Missing #root element");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
