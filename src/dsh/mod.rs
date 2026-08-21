/// dsh 桥：接收 deepseek-harness 插件推送的任务事件（loopback HTTP 直推）。
///
/// ZapMomo 在 app 进程内起一个仅绑 127.0.0.1 的极小 HTTP 服务；dsh 侧 Cordis 插件
/// 在任务状态翻转瞬间 POST 语义化事件（`POST /dsh/events` + Bearer token），毫秒级
/// 到达、无轮询。端口与 token 写入发现文件 `~/.zapmomo/runtime/dsh-bridge.json`
/// （权限 0600），插件每次发送前现读；ZapMomo 未运行时插件静默跳过。
pub mod config;
pub mod event;
