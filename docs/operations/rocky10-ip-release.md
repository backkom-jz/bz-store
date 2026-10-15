# Rocky Linux 10 IP 预发布

在腾讯云主机 `VM-0-9-rockylinux` 上用宿主机 Nginx 发布当前仓库。公网地址以元数据为准，本次是 `81.69.13.83`。MySQL 使用云数据库 8.0，对象存储使用阿里云华东 2 的私有桶。应用目录是 `/var/www/html/bz-store`，源码目录是 `/usr/local/src/bz-store`。不要在发布目录里编译，不要在这台机器上运行 Docker、MySQL 服务端、MinIO 或 ClamAV。

这是 HTTP 预发布，不是正式上线。`APP_ENV=test`，数据库只能是隔离的 `bz_ztore_test`。安全组的 80 端口只放行验收者来源 IP。不要使用生产库 `bz_ztore` 或 ERP 库，不要登记真实账号、真实付款或个人资料。

正式上线另见文末「切到 store.bengzi.cn」。只给 IP 配 HTTP 不能满足生产配置：生产进程要求 `APP_ENV=production`、`SITE_URL=https://store.bengzi.cn`、HTTPS 对象存储和 `MFA_ENCRYPTION_KEY`。

相关说明：[邮箱与管理员 MFA](auth-email-mfa.md)、[MySQL 备份](mysql-backups.md)、[上传文件](uploads.md)。

## 0. 发布前核对

在动手前逐项确认。任一项不成立就停止。

| 检查 | 要求 |
| --- | --- |
| 系统 | Rocky Linux 10、x86_64。`ldd --version` 显示 glibc 2.39 |
| 内存 | 约 2 GB。本机编译前先加 4 GB swap |
| 构建位置 | 源码在 `/usr/local/src/bz-store`。`/var/www/html/bz-store` 只放发布产物 |
| 公网地址 | `curl -4 -fsS http://metadata.tencentyun.com/latest/meta-data/public-ipv4` 的结果。本次为 `81.69.13.83`。`SITE_URL` 和 Nginx `server_name` 必须使用这个地址 |
| 数据库 | 云数据库 MySQL 8.0 上已有空库 `bz_ztore_test`，账号只能访问这个库 |
| 对象存储 | 阿里云桶 `bz-store`，地域华东 2（上海），私有，阻止公共访问。密钥只属于这只桶 |
| 入站 | 云安全组只放行验收者访问 TCP 80。不放行 3000、18080、6379、3306 |
| 邮件 | 本阶段不配置 SMTP。注册和找回密码会不可用，不要把这两项算作通过 |

API 监听 `0.0.0.0:18080`，Nuxt 和 Redis 只监听 `127.0.0.1`。18080 能起来不代表可以暴露；主机防火墙和安全组都必须拒绝它。

## 1. 准备主机

以 root 执行。`mariadb` 这个包只提供连接云数据库的 `mysql` 客户端，不要安装 `mariadb-server`，也不要在本机启动数据库。

```bash
dnf -y install nginx redis policycoreutils-python-utils tar xz mariadb bind-utils
timedatectl set-timezone Asia/Shanghai
command -v mysql
```

`command -v` 应打印 `/usr/bin/mysql`。

安装 Node.js 24，并核对官方校验和。Nuxt 直接调用 `/opt/node/bin/node`，不依赖登录 shell 的 PATH。版本以 [Node.js 24 归档](https://nodejs.org/en/download/archive/v24) 为准；升级前先在预发布验证。

```bash
cd /tmp
NODE_VERSION=24.21.0
curl -fSLO "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz"
curl -fSLO "https://nodejs.org/dist/v${NODE_VERSION}/SHASUMS256.txt"
grep " node-v${NODE_VERSION}-linux-x64.tar.xz$" SHASUMS256.txt | sha256sum -c -
tar -xJf "node-v${NODE_VERSION}-linux-x64.tar.xz" -C /opt
ln -sfn "/opt/node-v${NODE_VERSION}-linux-x64" /opt/node
/opt/node/bin/node --version
```

`sha256sum -c` 必须打印 `OK`。

Redis 只接受本机连接：

```bash
install -d -o redis -g redis -m 0750 /var/lib/redis
sed -i 's/^bind .*/bind 127.0.0.1 -::1/' /etc/redis/redis.conf
sed -i 's/^protected-mode .*/protected-mode yes/' /etc/redis/redis.conf
sed -i 's/^supervised .*/supervised systemd/' /etc/redis/redis.conf
systemctl enable --now redis
redis-cli -h 127.0.0.1 ping
```

期望输出 `PONG`。若配置路径不是 `/etc/redis/redis.conf`，用 `rpm -qc redis` 找到实际文件后再改这三项。

防火墙只开放 HTTP：

```bash
firewall-cmd --permanent --add-service=http
firewall-cmd --reload
firewall-cmd --list-services
```

列表里应有 `http`，不应为本次发布新增数据库或应用端口。云安全组再限制 80 的来源为验收者 IP。

SELinux 保持 Enforcing。Nginx 要反代本机 API 和 Nuxt，并读取后台静态文件：

```bash
getenforce
setsebool -P httpd_can_network_connect 1
```

创建运行用户和目录：

```bash
useradd --system --home-dir /var/lib/devstore --create-home --shell /sbin/nologin devstore
install -d -o root -g root -m 0755 /var/www/html/bz-store/releases
install -d -o root -g devstore -m 0750 /etc/devstore
```

公网 IP 和 DNS。对象存储域名解析失败时，不要把地址写进 `/etc/hosts`。

```bash
curl -4 -fsS --max-time 3 http://metadata.tencentyun.com/latest/meta-data/public-ipv4
dig +time=2 +tries=1 oss-cn-shanghai.aliyuncs.com A
dig +time=2 +tries=1 @119.29.29.29 oss-cn-shanghai.aliyuncs.com A
dig +time=2 +tries=1 @223.5.5.5 oss-cn-shanghai.aliyuncs.com A
```

第一条应打印 `81.69.13.83`。后两条出现 `A` 记录、第一条没有时，是系统 DNS 不可用：

```bash
CON=$(nmcli -t -f NAME,DEVICE connection show --active | awk -F: '$2!="" && $2!="lo"{print $1; exit}')
nmcli connection modify "$CON" ipv4.ignore-auto-dns yes ipv4.dns "119.29.29.29 223.5.5.5"
nmcli connection up "$CON"
dig +time=2 +tries=1 oss-cn-shanghai.aliyuncs.com A
```

三条 `dig` 都超时，则是出站 UDP 53 被安全组或防火墙拦住。`curl: (6) Could not resolve host` 属于这一类，先修 DNS 再初始化桶。

## 2. 准备 MySQL 和对象存储

在云数据库上创建空库 `bz_ztore_test` 和专用账号。账号只授予这个库的 `SELECT`、`INSERT`、`UPDATE`、`DELETE`、`CREATE`、`ALTER`、`INDEX`、`REFERENCES`。迁移会建表，所以需要这些 DDL 权限。不要授予 `SUPER`，不要授权其他库。

这台云数据库当前没有向客户端提供 TLS。MariaDB 客户端不认识 MySQL 的 `ssl-mode`；加上 `--ssl-verify-server-cert` 会报 `SSL is required, but the server does not support it`。在控制台开通 SSL 之前，用下面的命令确认库名：

```bash
mysql --skip-ssl -h RDS_HOST -P 3306 -u TEST_USER -p bz_ztore_test -e 'SELECT VERSION(), DATABASE();'
```

期望看到 `8.0` 和 `bz_ztore_test`。连接失败时不要启动 API。

控制台开通 SSL 并把厂商 CA 保存为 `/etc/devstore/rds-ca.pem`（权限 `0644`）之后，再改用：

```bash
mysql --ssl-ca=/etc/devstore/rds-ca.pem --ssl-verify-server-cert \
  -h RDS_HOST -P 3306 -u TEST_USER -p bz_ztore_test -e 'SELECT DATABASE();'
```

这条成功之前，不要在 `DATABASE_URL` 里写 `ssl-mode=VERIFY_IDENTITY`。API 使用的 sqlx 认得这个参数；实例未开 SSL 时，进程会在连库阶段退出。开通并验证之后，再把查询串加回去：

```text
?ssl-mode=VERIFY_IDENTITY&ssl-ca=/etc/devstore/rds-ca.pem
```

密码里的 `@`、`:`、`/`、`?`、`#`、`%` 必须做 URL 编码后再放进 `DATABASE_URL`，分别写成 `%40`、`%3A`、`%2F`、`%3F`、`%23`、`%25`。不要用 shell 的 `source` 加载 `api.env`，`&` 会被拆开。

对象存储使用一对只属于测试桶的密钥。阿里云控制台里的对应关系：

| 控制台 | 环境变量 |
| --- | --- |
| Bucket 名称 `bz-store` | `S3_BUCKET=bz-store` |
| Endpoint `oss-cn-shanghai.aliyuncs.com` | `S3_ENDPOINT=https://oss-cn-shanghai.aliyuncs.com` |
| 地域华东 2（上海） | `S3_REGION=oss-cn-shanghai` |
| 私有、阻止公共访问 | 保持。不要改成公共读 |

端点必须带 `https://`，不要把桶名写进端点，不要使用内网地址 `oss-cn-shanghai-internal.aliyuncs.com`。这台机器不在阿里云 VPC 里。程序在 `S3_VIRTUAL_HOSTED=true` 时访问 `https://bz-store.oss-cn-shanghai.aliyuncs.com`。桶可以事先建好；`init-storage` 遇到已存在的桶会当作成功。

默认路径风格和区域 `us-east-1` 只适用于本地 MinIO。阿里云会返回 `SecondLevelDomainForbidden` / `Please use virtual hosted style to access`。旧二进制不读 `S3_VIRTUAL_HOSTED`，只改环境文件无效，必须重新编译。

先确认公网能连上端点。`403` 表示网络通、匿名访问被拒绝，这是预期：

```bash
curl -4 -sS -o /dev/null -w 'ipv4 %{http_code}\n' --max-time 8 https://oss-cn-shanghai.aliyuncs.com
```

## 3. 在源码目录编译并装进发布目录

`/var/www/html/bz-store` 只放发布产物，里面没有 `package.json`，也没有 `cargo`。在这个目录执行 `npm run build` 会报 `ENOENT`。不要把源码克隆到这个目录。

只有这一台 2 GB 机器可用时，先加 4 GB swap，否则 `cargo build --release` 容易被系统杀掉：

```bash
if ! swapon --show | grep -q .; then
  fallocate -l 4G /swapfile || dd if=/dev/zero of=/swapfile bs=1M count=4096
  chmod 600 /swapfile
  mkswap /swapfile
  swapon /swapfile
  grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap defaults 0 0' >> /etc/fstab
fi
```

安装编译依赖和 Rust 1.97.0。新开的 root shell 里没有 `cargo`，要先加载环境：

```bash
dnf -y install git gcc make pkgconf openssl-devel
curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97.0
. "$HOME/.cargo/env"
export PATH="/opt/node/bin:$HOME/.cargo/bin:$PATH"
node --version
cargo --version
```

`node` 应显示 v24，`cargo` 应显示 1.97.0。

把仓库放到源码目录。开发机上的分支若还没推到远端，就从开发机上传，不要克隆一个没有本次修正的旧分支：

```bash
# 在开发机的仓库根目录执行
rsync -a --delete \
  --exclude .git --exclude target --exclude node_modules \
  --exclude .runtime --exclude .nuxt --exclude .output --exclude dist \
  --exclude .env \
  ./ root@81.69.13.83:/usr/local/src/bz-store/
```

确认源码到位后再构建。迁移 SQL 会编译进二进制；改了 `server/migrations/` 之后必须重新 `cargo build`，只替换环境文件不会生效。

```bash
export PATH="/opt/node/bin:$HOME/.cargo/bin:$PATH"
cd /usr/local/src/bz-store
test -f package.json
test -f Cargo.lock
test -z "$(grep CLAMAV server/src/config.rs || true)"
grep -n 'CHECK(subtotal_cents=unit_price_cents\*quantity)' server/migrations/0001_core.sql
npm ci
npm run build --workspace @devstore/web
npm run build --workspace @devstore/admin
cargo build --locked --release --package devstore-server --bins
```

`grep CLAMAV` 必须没有输出。`CHECK` 那一行必须在 `FOREIGN KEY` 之后，不能写在 `subtotal_cents BIGINT` 同一列上。MySQL 8.0 会拒绝列级约束引用其他列，错误是 `ERROR 3813 ... references other column`。

产物在源码目录的 `target/release/`，不是 `bin/`：

```bash
test -x target/release/devstore-server
test -x target/release/admin
test -d apps/web/.output/server
test -f apps/admin/dist/index.html
file target/release/devstore-server
objdump -T target/release/devstore-server | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1
```

`file` 应显示 `ELF 64-bit LSB`、`x86-64`。最后一列 glibc 版本不得高于 `2.39`。

本机编译时不要执行 `tar -xzf`。直接装进新的发布目录：

```bash
cd /usr/local/src/bz-store
RELEASE_ID=$(date -u +%Y%m%dT%H%M%SZ)
ROOT="/var/www/html/bz-store/releases/${RELEASE_ID}"
install -d -m 0755 "$ROOT/bin" "$ROOT/web" "$ROOT/admin-dist"
cp -a target/release/devstore-server target/release/admin "$ROOT/bin/"
cp -a apps/web/.output "$ROOT/web/.output"
cp -a apps/admin/dist/. "$ROOT/admin-dist/"
echo "$RELEASE_ID"
```

记下 `RELEASE_ID`。只重编了 Rust 二进制时，页面目录可以从上一版复制，不必重跑 `npm`：

```bash
OLD=$(readlink -f /var/www/html/bz-store/current)
cp -a "$OLD/web/.output" "$ROOT/web/.output"
cp -a "$OLD/admin-dist/." "$ROOT/admin-dist/"
```

权限要同时满足两件事：`devstore` 能执行 API，`nginx` 能沿路径读到后台页面。

```bash
chown -R root:root "$ROOT"
find "$ROOT" -type d -exec chmod 0755 {} +
find "$ROOT/web" "$ROOT/bin" -exec chown root:devstore {} +
find "$ROOT/web" "$ROOT/bin" -type d -exec chmod 0750 {} +
find "$ROOT/bin" -type f -exec chmod 0750 {} +
find "$ROOT/web" -type f -exec chmod 0640 {} +
chmod 0750 "$ROOT/web" "$ROOT/bin"
find "$ROOT/admin-dist" -type d -exec chmod 0755 {} +
find "$ROOT/admin-dist" -type f -exec chmod 0644 {} +
sudo -u devstore test -x "$ROOT/bin/devstore-server"
sudo -u nginx test -r "$ROOT/admin-dist/index.html"
semanage fcontext -a -t httpd_sys_content_t '/var/www/html/bz-store/releases(/.*)?' || true
restorecon -Rv "$ROOT"
ln -sfn "$ROOT" /var/www/html/bz-store/current.new
mv -Tf /var/www/html/bz-store/current.new /var/www/html/bz-store/current
readlink -f /var/www/html/bz-store/current
```

两条 `test` 都没有输出。`readlink` 必须指向刚安装的 `RELEASE_ID`。`semanage` 报上下文已存在时，保留 `|| true`，继续 `restorecon`。发布目录里不能有 `.env`。

在另一台机器编译时，再打成归档上传到服务器的 `/tmp`。归档顶层必须是 `bin`、`web`、`admin-dist`。`test -f` 失败就不要运行 `tar`。

## 4. 写入配置

两个文件都由 root 所有，组为 `devstore`，模式 `0640`。不要提交到 Git。先写文件，再替换会话密钥。模板里的占位文字只有 27 字节，原样留下会被拒绝。

`/etc/devstore/api.env`：

```ini
APP_ENV=test
APP_PORT=18080
SITE_URL=http://81.69.13.83
DATABASE_URL=mysql://TEST_USER:URL编码后的密码@RDS_HOST:3306/bz_ztore_test
REDIS_URL=redis://127.0.0.1:6379
JWT_SECRET=替换成第一枚随机值
CSRF_SECRET=替换成另一枚随机值
S3_ENDPOINT=https://oss-cn-shanghai.aliyuncs.com
S3_ACCESS_KEY=测试访问密钥
S3_SECRET_KEY=测试秘密密钥
S3_BUCKET=bz-store
S3_REGION=oss-cn-shanghai
S3_VIRTUAL_HOSTED=true
TRUSTED_PROXIES=127.0.0.1
```

`SITE_URL` 不能带路径、查询或尾部斜杠，并且必须是浏览器里打开的那个公网地址。`APP_ENV=test` 时，`DATABASE_URL` 解析出的路径必须正好是 `/bz_ztore_test`，不能是 `/bz_ztore`、`/bz_store_test` 或 `/bz_ztore_test/`。协议必须是 `mysql`。不要写入 `MYSQL_TEST_DATABASE_URL`，API 不读这个变量。

云数据库开通 SSL 之后，把 `DATABASE_URL` 换成带查询串的形式，其他行不动：

```text
DATABASE_URL=mysql://TEST_USER:URL编码后的密码@RDS_HOST:3306/bz_ztore_test?ssl-mode=VERIFY_IDENTITY&ssl-ca=/etc/devstore/rds-ca.pem
```

SMTP 要么全部不写，要么同时写齐 `SMTP_HOST`、`EMAIL_FROM`、`EMAIL_OUTBOX_ENCRYPTION_KEY`，并按服务商要求成对填写 `SMTP_USERNAME` 和 `SMTP_PASSWORD`。只写其中一部分会让 API 无法启动。本阶段保持全部不写。

生成两枚不同的会话密钥，直接写入文件，不在终端显示内容：

```bash
python3 << 'PY'
import pathlib, secrets
path = pathlib.Path("/etc/devstore/api.env")
jwt = secrets.token_urlsafe(32)
csrf = secrets.token_urlsafe(32)
lines, seen = [], set()
for line in path.read_text().splitlines():
    key = line.split("=", 1)[0].removeprefix("export ").strip()
    if key == "JWT_SECRET":
        lines.append(f"JWT_SECRET={jwt}")
        seen.add(key)
    elif key == "CSRF_SECRET":
        lines.append(f"CSRF_SECRET={csrf}")
        seen.add(key)
    else:
        lines.append(line)
if "JWT_SECRET" not in seen:
    lines.append(f"JWT_SECRET={jwt}")
if "CSRF_SECRET" not in seen:
    lines.append(f"CSRF_SECRET={csrf}")
path.write_text("\n".join(lines) + "\n")
print("JWT_SECRET", len(jwt), "CSRF_SECRET", len(csrf), "different", jwt != csrf)
PY
```

输出应显示两个长度都不小于 32，并且 `different True`。不要把这两枚值复用为邮件或 MFA 密钥。

`/etc/devstore/web.env`：

```ini
NODE_ENV=production
HOST=127.0.0.1
PORT=3000
NITRO_HOST=127.0.0.1
NITRO_PORT=3000
NUXT_API_INTERNAL_URL=http://127.0.0.1:18080
NUXT_TELEMETRY_DISABLED=1
NODE_OPTIONS=--max-old-space-size=512
```

收紧权限，并只打印键名：

```bash
chown root:devstore /etc/devstore/api.env /etc/devstore/web.env
chmod 0640 /etc/devstore/api.env /etc/devstore/web.env
awk -F= '/^[[:space:]]*(export[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*=/ {
  key=$1; sub(/^[[:space:]]*export[[:space:]]+/, "", key); print key
}' /etc/devstore/api.env
```

至少要有 `APP_ENV`、`APP_PORT`、`SITE_URL`、`DATABASE_URL`、`REDIS_URL`、`JWT_SECRET`、`CSRF_SECRET`、`S3_ENDPOINT`、`S3_ACCESS_KEY`、`S3_SECRET_KEY`、`S3_BUCKET`、`S3_REGION`、`S3_VIRTUAL_HOSTED`。每行写成 `KEY=value`，不要在 `=` 两侧加空格。

只看库名和对象存储地址、不打印密码时：

```bash
python3 << 'PY'
import pathlib
from urllib.parse import urlparse
env = {}
for raw in pathlib.Path("/etc/devstore/api.env").read_text().splitlines():
    line = raw.strip()
    if not line or line.startswith("#") or "=" not in line:
        continue
    if line.startswith("export "):
        line = line[7:].lstrip()
    key, value = line.split("=", 1)
    env[key.strip()] = value
db = urlparse(env.get("DATABASE_URL", ""))
s3 = urlparse(env.get("S3_ENDPOINT", ""))
print("APP_ENV", env.get("APP_ENV"))
print("SITE_URL", env.get("SITE_URL"))
print("db_scheme", db.scheme)
print("db_path", db.path)
print("s3_host", s3.hostname)
print("s3_bucket", env.get("S3_BUCKET"))
print("s3_region", env.get("S3_REGION"))
print("s3_virtual", env.get("S3_VIRTUAL_HOSTED"))
PY
```

`db_path` 必须是 `/bz_ztore_test`，`s3_host` 必须是 `oss-cn-shanghai.aliyuncs.com`，`s3_virtual` 必须是 `true`。

## 5. 安装并启动服务

`/etc/systemd/system/devstore-api.service`：

```ini
[Unit]
Description=DevStore Rust API
After=network-online.target redis.service
Wants=network-online.target

[Service]
Type=simple
User=devstore
Group=devstore
WorkingDirectory=/var/www/html/bz-store/current
EnvironmentFile=/etc/devstore/api.env
Environment=RUST_LOG=info
ExecStart=/var/www/html/bz-store/current/bin/devstore-server
Restart=on-failure
RestartSec=5
LimitNOFILE=65536
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

`/etc/systemd/system/devstore-web.service`：

```ini
[Unit]
Description=DevStore Nuxt SSR
After=network-online.target devstore-api.service
Wants=network-online.target
Requires=devstore-api.service

[Service]
Type=simple
User=devstore
Group=devstore
WorkingDirectory=/var/www/html/bz-store/current
EnvironmentFile=/etc/devstore/web.env
ExecStart=/opt/node/bin/node /var/www/html/bz-store/current/web/.output/server/index.mjs
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true

[Install]
WantedBy=multi-user.target
```

API 先连 MySQL 并执行迁移，成功后才监听 18080。`systemctl is-active` 显示 `active` 只说明进程已启动，不说明端口已打开。迁移失败时进程会每 5 秒重启；看到 `database migration failed` 后立刻停下，不要让重启计数一直增加。

```bash
systemctl daemon-reload
systemctl enable --now devstore-api
sleep 5
systemctl is-active devstore-api
ss -lnt | awk 'NR==1 || /:18080/'
journalctl -u devstore-api -n 30 --no-pager
```

`ss` 里应有 `0.0.0.0:18080`。没有的话，以日志里的 panic 为准，修完再 `systemctl start devstore-api`。配置错误会写出原因，例如库名不对、密钥太短或 SMTP 不完整。

迁移 SQL 由编译进二进制的版本执行。MySQL 的建表不能回滚。失败后 `_sqlx_migrations.success` 为 `0`，已经建出的表会留下，继续重启不会从头重跑。空的 `bz_ztore_test` 可以删掉全部表，再启动含修正的二进制。下面的脚本会拒绝非 `bz_ztore_test` 的库：

```bash
systemctl stop devstore-api
python3 << 'PY'
import pathlib, subprocess, urllib.parse
env = {}
for raw in pathlib.Path("/etc/devstore/api.env").read_text().splitlines():
    line = raw.strip()
    if not line or line.startswith("#") or "=" not in line:
        continue
    if line.startswith("export "):
        line = line[7:].lstrip()
    key, value = line.split("=", 1)
    env[key.strip()] = value
url = urllib.parse.urlparse(env["DATABASE_URL"])
assert url.path == "/bz_ztore_test", url.path
cnf = pathlib.Path("/run/devstore-mysql.cnf")
cnf.write_text(
    "[client]\n"
    f"host={url.hostname}\n"
    f"port={url.port or 3306}\n"
    f"user={urllib.parse.unquote(url.username or '')}\n"
    f"password={urllib.parse.unquote(url.password or '')}\n"
    "skip-ssl\n"
)
cnf.chmod(0o600)
sql = """
SET FOREIGN_KEY_CHECKS=0;
SET GROUP_CONCAT_MAX_LEN=1000000;
SET @drop = (
  SELECT CONCAT('DROP TABLE IF EXISTS ', GROUP_CONCAT(CONCAT('`', table_name, '`') SEPARATOR ', '))
  FROM information_schema.tables
  WHERE table_schema = 'bz_ztore_test'
);
SET @drop = IFNULL(@drop, 'SELECT 1');
PREPARE stmt FROM @drop;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;
SET FOREIGN_KEY_CHECKS=1;
SHOW TABLES;
"""
try:
    subprocess.run(["mysql", f"--defaults-extra-file={cnf}", "bz_ztore_test"], input=sql, text=True, check=False)
finally:
    cnf.unlink(missing_ok=True)
PY
```

`SHOW TABLES` 应为空。然后 `systemctl start devstore-api`，再确认 18080 已监听。

初始化测试桶。环境文件由 systemd 读入，可重复执行。成功输出是 `Private storage initialized`：

```bash
systemd-run --uid=devstore --gid=devstore --pipe --wait \
  --property=EnvironmentFile=/etc/devstore/api.env \
  --property=WorkingDirectory=/var/www/html/bz-store/current \
  /var/www/html/bz-store/current/bin/admin init-storage
```

几十毫秒内退出并提示缺少某个配置，说明那一项在 `api.env` 里为空，进程还没访问网络。`缺少配置 CLAMAV_ADDR` 表示二进制仍是移除 ClamAV 之前的版本，不要安装 ClamAV，重新编译。`SecondLevelDomainForbidden` 表示还在用路径风格，确认 `S3_VIRTUAL_HOSTED=true` 并且二进制含这个开关。

然后启动页面。Nginx 把 `/` 转到 `127.0.0.1:3000`；web 没起来时，浏览器看到的是 502，错误日志是 `connect() failed (111: Connection refused)`。

```bash
curl -fsS http://127.0.0.1:18080/api/v1/health/live
curl -fsS http://127.0.0.1:18080/api/v1/health/ready
systemctl enable --now devstore-web
curl -fsS -o /dev/null -w 'web %{http_code}\n' http://127.0.0.1:3000/
ss -lnt | awk 'NR==1 || /:80|:3000|:18080|:6379/'
```

`live` 的 `data.status` 为 `alive`。`ready` 的 `data.status` 为 `ready`。Nuxt 返回 `200`。`ready` 同时检查 MySQL、Redis 和对象存储列举；503 时不要继续用浏览器验收。3000 和 6379 必须在 `127.0.0.1`。18080 会出现在 `0.0.0.0`，这是程序的绑定方式；它仍然不能出现在防火墙或安全组的放行规则里。

## 6. 加载 Nginx

写入 `/etc/nginx/conf.d/store.conf`。`server_name` 使用第 1 步确认的公网地址。`/admin/` 的回退使用命名 location，避免 `alias` 和 `try_files` 组合找不到页面。

```nginx
server {
    listen 80;
    server_name 81.69.13.83;
    client_max_body_size 6m;
    add_header X-Content-Type-Options nosniff always;
    add_header Referrer-Policy strict-origin-when-cross-origin always;

    location /api/ {
        proxy_pass http://127.0.0.1:18080;
        proxy_http_version 1.1;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 45s;
        proxy_no_cache 1;
        error_page 413 = @too_large;
    }

    location @too_large {
        default_type application/json;
        return 413 '{"code":"FILE_TOO_LARGE","message":"上传请求超过 6 MiB","data":null,"request_id":"$request_id"}';
    }

    location = /admin {
        return 308 /admin/;
    }

    location /admin/ {
        alias /var/www/html/bz-store/current/admin-dist/;
        try_files $uri $uri/ @devstore_admin_spa;
        add_header Cache-Control "private, no-store" always;
        add_header Content-Security-Policy "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'" always;
        add_header X-Content-Type-Options nosniff always;
        add_header Referrer-Policy strict-origin-when-cross-origin always;
    }

    location @devstore_admin_spa {
        root /var/www/html/bz-store/current/admin-dist;
        try_files /index.html =404;
        add_header Cache-Control "private, no-store" always;
        add_header Content-Security-Policy "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'" always;
        add_header X-Content-Type-Options nosniff always;
        add_header Referrer-Policy strict-origin-when-cross-origin always;
    }

    location = /verify-email {
        access_log off;
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $http_host;
        proxy_set_header X-Forwarded-Proto $scheme;
        add_header Cache-Control "private, no-store" always;
    }

    location = /reset-password {
        access_log off;
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $http_host;
        proxy_set_header X-Forwarded-Proto $scheme;
        add_header Cache-Control "private, no-store" always;
    }

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_no_cache 1;
        add_header Cache-Control "private, no-store" always;
        add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'" always;
        add_header X-Content-Type-Options nosniff always;
        add_header Referrer-Policy strict-origin-when-cross-origin always;
    }
}
```

确认没有第二个 `server` 使用同一个 `server_name`，然后加载：

```bash
nginx -t
nginx -T | grep -n 'server_name 81.69.13.83'
systemctl enable --now nginx
systemctl reload nginx
```

`grep` 只应出现一处。从服务器本机验收：

```bash
curl -fsS -o /dev/null -w 'home %{http_code}\n' http://127.0.0.1/ -H 'Host: 81.69.13.83'
curl -fsS -o /dev/null -w 'admin %{http_code}\n' http://127.0.0.1/admin/ -H 'Host: 81.69.13.83'
curl -fsS -o /dev/null -w 'admin-deep %{http_code}\n' http://127.0.0.1/admin/products/new -H 'Host: 81.69.13.83'
curl -fsS http://127.0.0.1/api/v1/health/ready -H 'Host: 81.69.13.83'
```

四个结果都应为成功：页面 `200`，`ready` 的状态为 `ready`。后台深链接必须返回 HTML，而不是 Nginx 404。然后从验收者网络访问 `http://81.69.13.83/`，确认安全组来源限制仍然有效。

## 7. 初始化管理员和测试数据

没有默认管理员密码。密码长度 12–128，只从标准输入传入：

```bash
printf '%s\n' '替换成一次性管理员密码' | systemd-run --uid=devstore --gid=devstore --pipe --wait \
  --property=EnvironmentFile=/etc/devstore/api.env \
  --property=WorkingDirectory=/var/www/html/bz-store/current \
  /var/www/html/bz-store/current/bin/admin create --email admin@example.com --password-stdin
```

期望输出 `Admin created:` 和邮箱。重复创建同一邮箱会失败，不要为了重试去改生产库。

示例教程只允许出现在测试库。需要演示目录时才执行：

```bash
systemd-run --uid=devstore --gid=devstore --pipe --wait \
  --property=EnvironmentFile=/etc/devstore/api.env \
  --property=WorkingDirectory=/var/www/html/bz-store/current \
  /var/www/html/bz-store/current/bin/admin seed-demo --admin-email admin@example.com
```

收款配置单独放在 `/etc/devstore/payment.json`，权限 `0640`，所有者 `root:devstore`。预发布使用虚构账户：

```json
{
  "channel": "bank",
  "payee": "测试收款人",
  "account": "TEST-ONLY-NOT-A-REAL-ACCOUNT",
  "instructions": "仅用于预发布验收，不要转账。"
}
```

`channel` 只能是 `bank`、`wechat` 或 `alipay`。

```bash
systemd-run --uid=devstore --gid=devstore --pipe --wait \
  --property=EnvironmentFile=/etc/devstore/api.env \
  --property=WorkingDirectory=/var/www/html/bz-store/current \
  /var/www/html/bz-store/current/bin/admin payment-config \
  --admin-email admin@example.com --file /etc/devstore/payment.json
```

未写入收款配置时，下单返回「尚未配置收款信息，暂时不能购买」。这是预期行为，不是故障。

管理员 MFA 在预发布可以稍后做。正式切生产之前必须完成，因为生产进程缺少 `MFA_ENCRYPTION_KEY` 会拒绝启动。步骤见 [邮箱与管理员 MFA](auth-email-mfa.md)。在本机把其中的 `docker compose exec … admin` 换成上面的 `systemd-run … bin/admin`，后面仍接 `mfa-begin` 或 `mfa-confirm`。验证器种子和恢复码只在终端显示一次，不要送进网页、工单或聊天。

## 8. 验收记录

用测试账号完成下面各项，并把结果记在变更单里。不要上传真实付款凭证。

| 项目 | 通过标准 |
| --- | --- |
| 首页 | `http://81.69.13.83/` 返回店铺页面 |
| 后台入口 | `/admin/` 和 `/admin/products/new` 返回后台页面，刷新不落到 Nginx 404 |
| 健康检查 | `/api/v1/health/ready` 为 ready |
| 注册 | 未配置 SMTP 时注册失败并说明邮件不可用。不要改用生产邮件账号来“补测” |
| 管理员登录 | 使用第 7 步创建的账号进入后台 |
| 示例商品 | 若执行了 `seed-demo`，能看到示例教程，页面标明演示内容 |
| 未配置收款 | 不调用 `payment-config` 时，购买被拒绝 |
| 测试收款 | 配置虚构账户后，只用测试订单走下单、上传一张自备 PNG、驳回、重提、审核 |
| 凭证 | 只接受 PNG/JPEG，最大 5 MiB。当前不扫描恶意软件，审核时在后台查看，不打开图片里的链接 |
| 隔离 | 另一测试账号不能阅读未购买章节 |

付款凭证的处理边界见 [上传文件](uploads.md)。

## 9. 更新与回滚

每次更新都使用新的 `RELEASE_ID`。先按 [MySQL 备份](mysql-backups.md) 备份 `bz_ztore_test` 并确认能看到备份文件，再切换 `current`。不要覆盖正在使用的发布目录。改过迁移的二进制还要先处理 `_sqlx_migrations` 里 `success=0` 的半成品；空测试库按第 5 节删表后再启动。

```bash
ln -sfn "/var/www/html/bz-store/releases/${RELEASE_ID}" /var/www/html/bz-store/current.new
mv -Tf /var/www/html/bz-store/current.new /var/www/html/bz-store/current
systemctl restart devstore-api
sleep 5
ss -lnt | awk 'NR==1 || /:18080/'
curl -fsS http://127.0.0.1:18080/api/v1/health/ready
systemctl restart devstore-web
curl -fsS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:3000/
```

`ready` 不是 `ready`，或 Nuxt 不是 `200` 时，立刻切回上一版并重启两个服务：

```bash
ln -sfn /var/www/html/bz-store/releases/上一版本 /var/www/html/bz-store/current.new
mv -Tf /var/www/html/bz-store/current.new /var/www/html/bz-store/current
systemctl restart devstore-api devstore-web
```

迁移不会自动反向执行。切回旧程序前，确认旧程序仍能读取这次迁移后的表；不能读取时，恢复到本次发布前的数据库备份，而不是只换二进制。至少保留当前版和上一版两个目录。

查看运行状态：

```bash
systemctl status devstore-api devstore-web nginx redis --no-pager
journalctl -u devstore-api -u devstore-web --since '30 min ago' --no-pager
tail -n 100 /var/log/nginx/error.log
```

| 现象 | 先看 |
| --- | --- |
| 几十毫秒内 `缺少配置 …` | `api.env` 里该键为空。只打印键名，不要 `cat` 这个文件 |
| `JWT_SECRET 与 CSRF_SECRET 必须不同且至少 32 字节` | 用第 4 节的 Python 重写两枚密钥。占位汉字不够 32 字节 |
| `DATABASE_URL 必须指向 DevStore 对应的 MySQL 数据库` | 路径必须正好是 `/bz_ztore_test`。密码里的保留字符要 URL 编码 |
| `缺少配置 CLAMAV_ADDR` | 二进制过旧。重新 rsync 并编译，不要安装 ClamAV |
| `无法连接对象存储: error sending request` | DNS 或端点缺 `https://`。先 `dig` 和 `curl -4` 阿里云端点 |
| `SecondLevelDomainForbidden` | 设置 `S3_VIRTUAL_HOSTED=true`、`S3_REGION=oss-cn-shanghai`，并使用含该开关的二进制 |
| `curl: (6) Could not resolve host` | 系统 DNS 失败。改用 `119.29.29.29` 和 `223.5.5.5`，不要写 `/etc/hosts` |
| `mysql: unknown variable 'ssl-mode=...'` | Rocky 的客户端是 MariaDB。检查用 `--skip-ssl` 或 `--ssl-verify-server-cert` |
| `SSL is required, but the server does not support it` | 云数据库还没打开 SSL。检查时用 `--skip-ssl`。开通前不要在 `DATABASE_URL` 里写 `VERIFY_IDENTITY` |
| API 反复重启，`database migration failed` | `systemctl stop devstore-api`。MySQL 8.0 的 `ERROR 3813` 是列级 `CHECK` 引用了其他列。空的 `bz_ztore_test` 删掉全部表后，用修正过的二进制重跑 |
| `active` 但 `curl` 连不上 18080 | 迁移还没结束，或进程刚退出。等 5 秒后看 `ss` 和 `journalctl`，不要连续重启 |
| 浏览器 HTTP 502 | Nginx 在转发 `127.0.0.1:3000`，`devstore-web` 没在监听。先让 `ready` 返回 200，再启动 web |
| `ready` 为 503 | MySQL、Redis，或测试桶尚未 `init-storage` |
| `/admin/products/new` 为 404 | `current/admin-dist/index.html` 是否存在，`nginx` 用户能否读取，命名 location 是否仍在配置里 |
| 注册不可用 | 本阶段未配置 SMTP，这是预期结果 |

## 10. 切到 store.bengzi.cn

下面全部完成后，才把预发布换成生产。缺一项就保持第 6 步的 IP 配置。

1. `store.bengzi.cn` 的 A 记录指向当前公网地址 `81.69.13.83`，证书是该域名的有效 `fullchain.pem` 和 `privkey.pem`。
2. 在云数据库控制台开通 SSL，并用 `--ssl-verify-server-cert` 确认客户端能连上。生产 `DATABASE_URL` 使用 `ssl-mode=VERIFY_IDENTITY`。
3. 新建空库 `bz_ztore` 和只属于它的账号。不要把 `bz_ztore_test` 改名当成生产库，也不要把测试密钥拷进生产。
4. 备份目标库，并按 [MySQL 备份](mysql-backups.md) 做一次隔离恢复演练。对象存储使用独立的生产桶和独立密钥。阿里云桶继续使用虚拟主机风格和真实区域。
5. 更换 `JWT_SECRET`、`CSRF_SECRET`。另生成 `MFA_ENCRYPTION_KEY`（`openssl rand -base64 32`，解码后必须是 32 字节）。生产不写这枚密钥时，API 拒绝启动。
6. 写齐 SMTP，否则注册和找回密码仍然不可用。邮件密钥不要复用 JWT、CSRF 或 MFA 密钥。
7. 把 `APP_ENV` 改为 `production`，`SITE_URL` 改为 `https://store.bengzi.cn`，`DATABASE_URL` 的库名改为 `bz_ztore`，`S3_ENDPOINT` 保持 `https://`。其他值不满足时进程不会进入生产模式。
8. Nginx 改为该域名的 443，并把 80 重定向到 HTTPS。保留本文的反代、后台回退、6 MiB 限制和 `/verify-email`、`/reset-password` 不记录访问日志。
9. 重新执行第 5 步的健康检查和第 8 步的验收。然后创建生产管理员并完成 MFA。不要在生产库执行 `seed-demo`。

完成第 9 步之前，安全组不要对公网放开 80/443。
