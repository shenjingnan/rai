import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("找不到根元素 #root");

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
