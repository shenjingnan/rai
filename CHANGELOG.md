# Changelog

## [0.1.1](https://github.com/shenjingnan/zapmomo/compare/v0.1.0...v0.1.1) - 2026-08-13

### Added

- *(docs)* 新增 Fumadocs 中文文档站并部署到 Cloudflare Pages ([#10](https://github.com/shenjingnan/zapmomo/pull/10))

### Other

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
