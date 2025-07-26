import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

import "./index.css";
import { ThemeProvider } from "./components/theme-provider";

const root = document.getElementById("root");

if (!root) {
  throw new Error("No root element found");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
