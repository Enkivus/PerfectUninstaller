import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

const params = new URLSearchParams(window.location.search);

if (import.meta.env.DEV && params.has("mock")) {
  const { installMock } = await import("./lib/mock");
  installMock();
}

function AutoOpen() {
  const stage = params.get("stage");
  useEffect(() => {
    if (!stage) return;
    let tries = 0;
    const tick = setInterval(() => {
      tries += 1;
      const first = document.querySelector<HTMLElement>(".app-row");
      if (first) {
        first.click();
        clearInterval(tick);
        setTimeout(() => {
          const btn = Array.from(
            document.querySelectorAll<HTMLElement>(".btn"),
          ).find((b) => b.textContent?.startsWith("Uninstall"));
          if (!btn) return;
          btn.click();
          if (stage === "final") {
            setTimeout(() => {
              const cont = Array.from(
                document.querySelectorAll<HTMLElement>(".btn"),
              ).find((b) => b.textContent?.includes("Continue"));
              cont?.click();
            }, 600);
          }
        }, 2200);
      } else if (tries > 40) {
        clearInterval(tick);
      }
    }, 150);
    return () => clearInterval(tick);
  }, []);
  return null;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
    <AutoOpen />
  </React.StrictMode>,
);