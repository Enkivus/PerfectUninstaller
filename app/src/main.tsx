import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

const isDev = import.meta.env.DEV;
const params = new URLSearchParams(window.location.search);
const useMock = isDev && params.has("mock");

if (useMock) {
  const { installMock } = await import("./lib/mock");
  installMock();
}

/**
 * Dev-only driver so the UI can be inspected at any stage without touching the
 * filesystem. `npm run dev`, then open `/?mock=1&stage=plan|final|report`.
 */
function AutoOpen() {
  const stage = params.get("stage");
  useEffect(() => {
    if (!useMock || !stage) return;

    const byText = (sel: string, text: string) =>
      Array.from(document.querySelectorAll<HTMLElement>(sel))
        .find((el) => el.textContent?.includes(text))
        ?.click();

    let tries = 0;
    const tick = setInterval(() => {
      if (++tries > 40) return clearInterval(tick);
      const first = document.querySelector<HTMLElement>(".app-row");
      if (!first) return;
      clearInterval(tick);
      first.click();
      if (stage === "plan") return;

      setTimeout(() => {
        byText(".btn", "Uninstall");
        setTimeout(() => {
          byText(".btn", "Continue");
          if (stage !== "report") return;
          setTimeout(() => {
            document.querySelector<HTMLElement>(".check-row .check")?.click();
            const input = document.querySelector<HTMLInputElement>(".typing-input");
            if (input) {
              const setter = Object.getOwnPropertyDescriptor(
                window.HTMLInputElement.prototype,
                "value",
              )?.set;
              setter?.call(input, "Spotify");
              input.dispatchEvent(new Event("input", { bubbles: true }));
            }
            setTimeout(() => byText(".btn", "Delete forever"), 300);
          }, 500);
        }, 700);
      }, 2200);
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