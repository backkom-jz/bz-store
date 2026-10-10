# 邮箱与管理员双重验证

开发和 CI 可以连接隔离的 Mailpit。它只用于接收测试邮件；生产使用 SMTP 提交服务，必须通过 TLS。SMTP 账号还未配置时，注册接口明确返回服务不可用，密码找回保持相同的通用响应且不发送邮件。

配置邮件需要 `SMTP_HOST`、`SMTP_PORT`、`EMAIL_FROM` 和独立的 `EMAIL_OUTBOX_ENCRYPTION_KEY`。账号密码由提供商要求时再填 `SMTP_USERNAME` 与 `SMTP_PASSWORD`。用 `openssl rand -base64 32` 生成 outbox 密钥，不要复用 JWT、CSRF 或 MFA 密钥。发送任务和用户 token 在同一 MySQL 事务写入；outbox 正文使用 AES-256-GCM 加密，成功投递后清除密文。配置变更或轮换 outbox 密钥前，先等待待发队列清空。

管理员账号创建后必须先在服务器终端初始化 MFA。将命令输出的 TOTP seed 加入验证器，并把恢复码离线保存；这些内容只显示一次：

```sh
docker compose --env-file .env -f deploy/docker-compose.yml exec api \
  admin mfa-begin --admin-email admin@example.com
printf '%s\n' '123456' | docker compose --env-file .env -f deploy/docker-compose.yml exec -T api \
  admin mfa-confirm --admin-email admin@example.com --code-stdin
```

设置 `MFA_ENCRYPTION_KEY` 为另一条独立的 32 字节 Base64 密钥。TOTP seed 以 AES-256-GCM 加密保存；恢复码只存 SHA-256 摘要，每条只能用一次。重新执行 `mfa-begin` 会撤销该管理员的旧会话和恢复码。管理 API 要求近 5 分钟内完成过 TOTP 或恢复码验证；普通用户 API 不受影响。不要通过网页、工单或聊天发送 seed 与恢复码。

验证邮件和密码重置页面会立即从浏览器地址栏移除一次性 token，Nginx 不记录这两个页面的请求 URI，并发送 `Cache-Control: private, no-store`。邮件链接始终使用配置的固定 `SITE_URL`，不会根据请求 Host 生成。
