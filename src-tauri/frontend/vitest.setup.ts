import "@testing-library/jest-dom/vitest";

// pixi-live2d-display/cubism4 在模块顶层检查 window.Live2DCubismCore，
// 测试环境不加载 index.html 里的 Cubism Core script，这里 mock 一个占位全局
// 以通过其模块加载检查（测试不实际渲染模型，不会触发真实 runtime 调用）。
if (typeof window !== "undefined") {
  (window as unknown as { Live2DCubismCore: unknown }).Live2DCubismCore ??= {};
}
