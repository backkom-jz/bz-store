# MySQL 备份与隔离恢复

生产备份使用 `mysqldump` 的一致性快照，并通过 `age` 公钥加密。生产主机只需要备份公钥 `AGE_RECIPIENT`；解密私钥应离线保存或放在独立备份账户中。脚本不会把密码放在进程参数或日志中，会在权限为 `0600` 的临时客户端配置中短暂保存，并在退出时删除。

```sh
DATABASE_URL='mysql://…/bz_ztore' \
AGE_RECIPIENT='age1…' \
BACKUP_DIR=/var/backups/devstore \
scripts/backup/mysql-backup.sh
```

备份目录必须位于独立磁盘或异地同步目标，并按组织保留策略加密保留；对象存储文件需要单独做版本或复制备份。至少每季度把一个备份恢复到专用的 `*_restore_test` 数据库，抽查商品、订单、权益和内容行数，再删除该隔离库。恢复脚本会拒绝运行库名 `bz_ztore`、ERP 名称及非 MySQL URL：

```sh
RESTORE_DATABASE_URL='mysql://…/bz_ztore_restore_test' \
scripts/backup/mysql-restore-test.sh /path/to/devstore-<timestamp>.sql.gz.age /secure/path/backup-identity.txt
```

恢复凭据必须只拥有隔离恢复库的权限。脚本不会创建目标数据库，也不应向生产 DSN 授予删除或建库权限。首次部署前需由运维人员建立隔离恢复库、配置备份复制和告警，并完成一次真实恢复演练。

本地站点通过 hosts 映射 `127.0.0.1 bz-store.local` 访问 `http://bz-store.local`。公网 Compose 覆盖层服务 `store.bengzi.cn`，要求外部提供有效证书文件 `fullchain.pem` 与 `privkey.pem`，并将 80 端口重定向到 HTTPS。真实 DNS 和证书签发仍需在目标主机上验收。
