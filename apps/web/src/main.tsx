import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@fontsource-variable/noto-sans-sc";
import { LocalizedSurface, UiLanguageProvider } from "./i18n/uiLanguage";
import { App } from "./pages/App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <UiLanguageProvider>
      <LocalizedSurface>
        <App />
      </LocalizedSurface>
    </UiLanguageProvider>
  </React.StrictMode>,
);
