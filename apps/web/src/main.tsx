import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@fontsource-variable/noto-sans-sc";
import { LocalizedSurface, UiLanguageProvider } from "./i18n/uiLanguage";
import { App } from "./pages/App";
import "./styles/foundation.css";
import "./styles/communication-legacy.css";
import "./styles/feature-legacy.css";
import "./styles/theme-core.css";
import "./styles/theme-contrast.css";
import "./styles/auth-and-console.css";
import "./styles/communication.css";
import "./styles/communication-project-context.css";
import "./styles/compact-workbench.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <UiLanguageProvider>
      <LocalizedSurface>
        <App />
      </LocalizedSurface>
    </UiLanguageProvider>
  </React.StrictMode>,
);
