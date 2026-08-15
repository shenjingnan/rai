import { getCurrentWindow } from "@tauri-apps/api/window";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { PetApp } from "./pet/PetApp";
import "./index.css";

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("找不到根元素 #root");

// 同一份 bundle 按窗口 label 分叉渲染：pet 窗口渲染桌宠，其余（main）渲染设置面板。
const isPet = getCurrentWindow().label === "pet";

createRoot(rootElement).render(
  <StrictMode>
    <ErrorBoundary>{isPet ? <PetApp /> : <App />}</ErrorBoundary>
  </StrictMode>,
);
