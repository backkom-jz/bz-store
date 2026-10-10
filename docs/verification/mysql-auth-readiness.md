# MySQL 与账号安全改造验证记录

日期：2026-10-10。此记录对应当前 MySQL 分支；历史 PostgreSQL 验收见 `tutorial-checkout.md`，不能替代本记录中的 MySQL 集成验收。

## 已通过

- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`。
- Rust 单元测试 3 项；配置测试 3 项；隔离数据库 URL 正反例 2 项；PostgreSQL 语法残留扫描 1 项。
- `scripts/test-deploy-config.sh`、`scripts/backup/test_mysql_defaults.py`、Node 脚本语法、Shell 脚本语法和 `git diff --check`。
- Node 24 下 `npm run typecheck`、`npm run test:unit`（5 个文件、16 项）和 `npm run build`（管理端 Vite、前台 Nuxt）。
- 独立只读复审确认六项问题已修复：重置密码资源消耗与限流、E2E 管理员初始化顺序、后台静态资源发布、生产 MySQL TLS 身份校验、备份 GTID、未知 Host 重定向。复审未连接 MySQL。

## 尚未执行

本机没有隔离的 `bz_ztore_test` 连接配置，因此没有运行 MySQL migration、真实事务/并发测试、完整 Rust 集成套件或 Playwright E2E。集成测试现在在缺少测试 DSN 时会明确失败，不再静默跳过。不能据此声称 MySQL schema 或购买闭环已通过运行时验收。

恢复完整验收前，先轮换之前暴露的数据库凭据，再把独立测试库 DSN 配入本机未提交的 `.env`（只能指向精确的 `bz_ztore_test`）。不得使用运行库 `bz_ztore`、ERP 数据库或本机 MySQL。CI 的临时 MySQL 服务也必须在 GitHub Actions 实际运行后确认绿灯。

## 部署边界

当前 MySQL migration 从空库初始化；不支持把已有 PostgreSQL 数据库原地升级。已有数据需先单独设计并验证导出、字段映射、导入和业务数据校验。公网 DNS、TLS 证书、RDS 权限/版本、SMTP 服务商账号和真实备份恢复演练仍需部署环境验收。
