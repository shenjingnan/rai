# Changelog

## [0.1.1](https://github.com/shenjingnan/rai/compare/v0.1.0...v0.1.1) - 2026-08-12

### Added

- *(desktop)* 引入 Tauri 2 桌面应用与三平台发布流水线 ([#4](https://github.com/shenjingnan/rai/pull/4))
- *(kws)* 接入 sherpa-onnx 唤醒词检测 ([#3](https://github.com/shenjingnan/rai/pull/3))

### Fixed

- *(release)* 对齐 rai-app publish 配置修复 release-plz 发布 ([#7](https://github.com/shenjingnan/rai/pull/7))

### Other

- 全面重命名项目为 RAI ([#6](https://github.com/shenjingnan/rai/pull/6))
- *(deps)* bump thiserror from 2.0.18 to 2.0.20 ([#1](https://github.com/shenjingnan/rai/pull/1))
- *(deps)* bump serde from 1.0.228 to 1.0.229 ([#2](https://github.com/shenjingnan/rai/pull/2))
- Initial commit

## [0.1.0] - 2026-06-05

### Added

- 项目初始化
- CLI 骨架（clap + tokio）
- 配置管理（TOML 配置读写）
- 双层日志系统（tracing）
- 日期时间工具模块
- CI/CD 配置（GitHub Actions）
- 代码质量工具（fmt, clippy, typos, tarpaulin, codecov）
- Shell 补全生成
