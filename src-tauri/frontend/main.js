// KWS 控制面板前端：通过 window.__TAURI__（withGlobalTauri）与 Rust 后端交互。
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);

/** 向 dl 追加一行配置（dt/dd），值一律用 textContent 防注入。 */
function addConfigRow(dl, label, value, mono = false) {
  const dt = document.createElement("dt");
  dt.textContent = label;
  const dd = document.createElement("dd");
  dd.textContent = value;
  if (mono) dd.classList.add("mono");
  dl.append(dt, dd);
}

/** 刷新麦克风设备下拉框。 */
async function refreshDevices() {
  const devices = await invoke("list_devices");
  const sel = $("device");
  sel.innerHTML = "";
  if (devices.length === 0) {
    const opt = document.createElement("option");
    opt.value = "";
    opt.textContent = "未找到输入设备";
    sel.appendChild(opt);
    sel.disabled = true;
  } else {
    for (const d of devices) {
      const opt = document.createElement("option");
      opt.value = d;
      opt.textContent = d;
      sel.appendChild(opt);
    }
    sel.disabled = false;
  }
}

/** 渲染 KWS 配置与模型状态。 */
async function refreshConfig() {
  const cfg = await invoke("get_kws_config");
  const dl = $("config");
  dl.innerHTML = "";
  addConfigRow(dl, "模型目录", cfg.model_dir, true);
  addConfigRow(dl, "后端 / 线程", `${cfg.provider} / ${cfg.num_threads}`);
  addConfigRow(dl, "采样率", String(cfg.sample_rate));
  addConfigRow(dl, "关键词", cfg.keywords.join("、") || "（空）", true);
  addConfigRow(dl, "配置路径", cfg.settings_path, true);

  const hint = $("model-hint");
  if (cfg.models_present) {
    hint.classList.add("hidden");
    $("model-progress").classList.add("hidden");
  } else {
    hint.classList.remove("hidden");
    $("model-hint-text").textContent =
      `⚠ 模型文件缺失（${cfg.model_dir}）。点击下方按钮下载后即可开始监听。`;
    $("download-model").disabled = cfg.model_downloading;
    if (cfg.model_downloading) {
      $("model-progress").classList.remove("hidden");
      $("model-progress-label").textContent = "正在下载...";
    }
  }
}

/** 更新模型下载进度条。 */
function setDownloadProgress(p) {
  const label = $("model-progress-label");
  $("model-progress").classList.remove("hidden");
  if (p.stage === "downloading") {
    const pct = Math.max(0, Math.min(100, p.percent));
    $("model-progress-bar").style.width = `${pct}%`;
  } else {
    $("model-progress-bar").style.width = "100%";
  }
  label.textContent = p.message;
}

function setListening(on) {
  $("status").textContent = on ? "监听中" : "空闲";
  $("status").className = on ? "status active" : "status idle";
  $("start").disabled = on;
  $("stop").disabled = !on;
}

async function updateIsListening() {
  setListening(await invoke("is_listening"));
}

/** 追加一条检测结果到日志顶部。 */
function appendResult(k) {
  const li = document.createElement("li");
  const time = new Date().toLocaleTimeString();
  li.textContent = `[${time}] ${k.keyword}（start=${k.start_time.toFixed(2)}s）`;
  $("results").prepend(li);
}

function showError(msg) {
  const el = $("listen-error");
  el.textContent = msg;
  el.classList.remove("hidden");
}

function init() {
  // 事件：后端检测到唤醒词 / 监听结束
  listen("kws-detected", (e) => appendResult(e.payload));
  listen("kws-stopped", (e) => {
    setListening(false);
    if (e.payload && e.payload.error) {
      showError(e.payload.error);
    }
  });

  // 模型下载进度事件
  listen("kws-model-download-progress", (e) => setDownloadProgress(e.payload));

  // 下载模型按钮：完成后刷新配置（models_present 变 true，按钮消失）
  $("download-model").addEventListener("click", async () => {
    const btn = $("download-model");
    btn.disabled = true;
    $("model-progress").classList.remove("hidden");
    $("model-progress-label").textContent = "开始下载...";
    try {
      await invoke("download_kws_model");
      await refreshConfig();
    } catch (e) {
      showError(String(e));
    } finally {
      btn.disabled = false;
      $("model-progress").classList.add("hidden");
      await refreshConfig();
    }
  });

  $("refresh-devices").addEventListener("click", () =>
    refreshDevices().catch(showError));

  $("start").addEventListener("click", async () => {
    $("listen-error").classList.add("hidden");
    try {
      await invoke("start_listen", {
        device: $("device").value || null,
        keywords: $("keywords").value || null,
      });
      setListening(true);
    } catch (e) {
      showError(String(e));
    }
  });

  $("stop").addEventListener("click", async () => {
    try {
      await invoke("stop_listen");
      setListening(false);
    } catch (e) {
      showError(String(e));
    }
  });

  // 初始化
  invoke("get_app_info")
    .then((info) => {
      $("app-version").textContent = `v${info.version}`;
      document.title = `${info.product_name} · KWS 控制面板`;
    })
    .catch(() => {});
  refreshDevices().catch(showError);
  refreshConfig().catch(showError);
  updateIsListening().catch(() => {});
}

init();
