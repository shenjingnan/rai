# Changelog

## [0.1.8](https://github.com/shenjingnan/zapmomo/compare/v0.1.7...v0.1.8) - 2026-08-14

### Added

- *(app)* macOS 窗口改用原生阴影并优化启动体验 ([#38](https://github.com/shenjingnan/zapmomo/pull/38))
- *(kws)* 英文关键词自动转 ARPAbet 音素 ([#36](https://github.com/shenjingnan/zapmomo/pull/36))

## [0.1.7](https://github.com/shenjingnan/zapmomo/compare/v0.1.6...v0.1.7) - 2026-08-13

### Fixed

- *(ci)* 修复 Release 工作流漏装 frontend 依赖导致构建失败 ([#34](https://github.com/shenjingnan/zapmomo/pull/34))

## [0.1.6](https://github.com/shenjingnan/zapmomo/compare/v0.1.5...v0.1.6) - 2026-08-13

### Added

- *(app)* 前端迁移到 React 并升级为无边框透明窗口 ([#31](https://github.com/shenjingnan/zapmomo/pull/31))

## [0.1.5](https://github.com/shenjingnan/zapmomo/compare/v0.1.4...v0.1.5) - 2026-08-13

### Other

- 更新 README，清理过时内容 ([#29](https://github.com/shenjingnan/zapmomo/pull/29))

## [0.1.4](https://github.com/shenjingnan/zapmomo/compare/v0.1.3...v0.1.4) - 2026-08-13

### Other

- *(ci)* Release 构建成功后自动发布，不再停留在草稿

## [0.1.3](https://github.com/shenjingnan/zapmomo/compare/v0.1.2...v0.1.3) - 2026-08-13

### Other

- 添加 ZapMomo logo 与 favicon ([#25](https://github.com/shenjingnan/zapmomo/pull/25))

## [0.1.2](https://github.com/shenjingnan/zapmomo/compare/v0.1.1...v0.1.2) - 2026-08-13

### Added

- *(kws)* 内置模型自动下载并修复打包路径失效 ([#23](https://github.com/shenjingnan/zapmomo/pull/23))

## [0.1.1](https://github.com/shenjingnan/zapmomo/compare/v0.1.0...v0.1.1) - 2026-08-13

### Added

- *(docs)* 新增 Fumadocs 中文文档站并部署到 Cloudflare Pages ([#10](https://github.com/shenjingnan/zapmomo/pull/10))

### Fixed

- *(ci)* 修复 release-plz 创建 Release PR 使用 PAT_TOKEN ([#21](https://github.com/shenjingnan/zapmomo/pull/21))

### Other

- *(deps)* bump cpal from 0.15.3 to 0.18.1 ([#14](https://github.com/shenjingnan/zapmomo/pull/14))
- *(deps)* bump actions/download-artifact from 4 to 8 ([#12](https://github.com/shenjingnan/zapmomo/pull/12))
- *(deps)* bump actions/cache from 4 to 6 ([#13](https://github.com/shenjingnan/zapmomo/pull/13))
- *(deps)* bump actions/setup-node from 4 to 7 ([#15](https://github.com/shenjingnan/zapmomo/pull/15))
- *(deps)* bump pnpm/action-setup from 4 to 6 ([#16](https://github.com/shenjingnan/zapmomo/pull/16))
- *(deps-dev)* bump @types/node from 22.20.1 to 26.2.0 ([#17](https://github.com/shenjingnan/zapmomo/pull/17))
- *(deps)* bump pinyin from 0.10.0 to 0.11.0 ([#18](https://github.com/shenjingnan/zapmomo/pull/18))
- *(deps-dev)* bump typescript from 5.9.3 to 7.0.2 ([#19](https://github.com/shenjingnan/zapmomo/pull/19))
- *(docs)* 部署切换为 Cloudflare Pages Git 集成并修复过期链接 ([#20](https://github.com/shenjingnan/zapmomo/pull/20))
- 从 npm 迁移到 pnpm ([#9](https://github.com/shenjingnan/zapmomo/pull/9))

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
