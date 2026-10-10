# Rocky Linux 10 IP 预发布

在 `124.221.135.64` 上用宿主机 Nginx 发布当前仓库。MySQL 使用外部 RDS，对象存储使用外部 HTTPS S3。应用目录是 `/var/www/html/bz-store`。不要在这台机器上编译，也不要运行 Docker、MySQL、MinIO 或 ClamAV。

这是 HTTP 预发布，不是正式上线。`APP_ENV=test`，数据库只能是隔离的 `bz_ztore_test`。安全组的 80 端口只放行验收者来源 IP。不要使用生产库 `bz_ztore` 或 ERP 库，不要登记真实账号、真实付款或个人资料。

正式上线另见文末「切到 store.bengzi.cn」。只给 IP 配 HTTP 不能满足生产配置：生产进程要求 `APP_ENV=production`、`SITE_URL=https://store.bengzi.cn`、HTTPS 对象存储和 `MFA_ENCRYPTION_KEY`。

相关说明：[邮箱与管理员 MFA](auth-email-mfa.md)、[MySQL 备份](mysql-backups.md)、[上传文件](uploads.md)。

## 0. 发布前核对

在动手前逐项确认。任一项不成立就停止。

| 检查 | 要求 |
| --- | --- |
| 系统 | Rocky Linux 10.2、x86_64。`ldd --version` 显示 glibc 2.39 |
| 内存 | 约 2 GB，只跑 Nginx、Redis、API 和 Nuxt |
| 构建机 | 源码放在 `/usr/local/src/bz-store`，不要放在 `/var/www/html/bz-store`。优先用另一台 glibc 不高于 2.39 的 x86_64 Linux；只有这一台机器时，先加 swap 再在本机编译 |
| 数据库 | 已存在空库 `bz_ztore_test`，账号只能访问这个库 |
| 对象存储 | 独立测试桶，HTTPS 端点，和 API 使用同一对密钥 |
| 入站 | 云安全组只放行验收者访问 TCP 80。不放行 3000、18080、6379、3306 |
| 邮件 | 本阶段不配置 SMTP。注册和找回密码会不可用，不要把这两项算作通过 |

API 监听 `0.0.0.0:18080`，Nuxt 和 Redis 只监听 `127.0.0.1`。18080 能起来不代表可以暴露；主机防火墙和安全组都必须拒绝它。

## 1. 准备主机

以 root 执行。

```bash
dnf -y install nginx redis policycoreutils-python-utils tar xz
timedatectl set-timezone Asia/Shanghai
```

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

列表里应有 `http`，不应为本次发布新增 `https` 以外的数据库或应用端口。云安全组再限制 80 的来源为验收者 IP。

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

## 2. 准备 MySQL 和对象存储

在 RDS 上创建空库和专用账号。账号只授予 `bz_ztore_test` 的 `SELECT`、`INSERT`、`UPDATE`、`DELETE`、`CREATE`、`ALTER`、`INDEX`、`REFERENCES`。迁移会建表，所以需要这些 DDL 权限。不要授予 `SUPER`，不要授权其他库。

下载 RDS 厂商的 CA，保存为 `/etc/devstore/rds-ca.pem`，权限 `0644`。先用客户端确认 TLS 和库名，再启动 API：

```bash
mysql --ssl-mode=VERIFY_IDENTITY --ssl-ca=/etc/devstore/rds-ca.pem \
  -h RDS_HOST -P 3306 -u TEST_USER -p bz_ztore_test -e 'SELECT DATABASE();'
```

期望输出 `bz_ztore_test`。连接失败时不要启动 API。

对象存储使用一对只属于测试桶的密钥。默认按路径风格访问 `S3_ENDPOINT`，签名区域默认 `us-east-1`，这是本地 MinIO 的用法。阿里云华东 2 拒绝路径风格，报 `SecondLevelDomainForbidden`。这只桶要额外写 `S3_VIRTUAL_HOSTED=true` 和 `S3_REGION=oss-cn-shanghai`。端点仍是 `https://oss-cn-shanghai.aliyuncs.com`，不要把桶名写进端点，程序会自己访问 `https://bz-store.oss-cn-shanghai.aliyuncs.com`。`S3_BUCKET` 写 `bz-store`。这台机器不在阿里云 VPC 内，不要使用 `oss-cn-shanghai-internal.aliyuncs.com`。桶可以事先建好并保持私有；`init-storage` 遇到已存在的桶会当作成功。

改这两个变量之前，服务器上的二进制必须是包含该开关的版本。旧二进制没有 `S3_VIRTUAL_HOSTED`，只改环境文件无效。`curl: (6) Could not resolve host` 是 DNS 失败，不要把地址写进 `/etc/hosts`。先用 `dig @119.29.29.29` 和 `dig @223.5.5.5` 查询 `oss-cn-shanghai.aliyuncs.com`。直连公共 DNS 能解析、系统解析不能时，把 NetworkManager 或 `systemd-resolved` 的 DNS 改成这两台。两边都超时则是出站 UDP 53 被安全组或防火墙拦住。

密码里的 `@`、`:`、`/`、`?`、`#` 必须做 URL 编码后再放进 `DATABASE_URL`。`@` 写成 `%40`。

## 3. 在源码目录打包

`/var/www/html/bz-store` 只放发布产物，里面没有 `package.json`，也没有 `cargo`。在这个目录执行 `npm run build` 会报 `ENOENT`，`cargo: command not found` 是因为服务器还没装 Rust。不要把源码克隆到这个目录。

源码目录固定为 `/usr/local/src/bz-store`。第 1 步的 Node 装在 `/opt/node`，默认不在 `PATH` 里。

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

安装编译依赖和 Rust 1.97.0：

```bash
dnf -y install git gcc make pkgconf openssl-devel
curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97.0
. "$HOME/.cargo/env"
export PATH="/opt/node/bin:$HOME/.cargo/bin:$PATH"
node --version
cargo --version
```

`node` 应显示 v24，`cargo` 应显示 1.97.0。把上面的 `export PATH` 留在当前 shell；新开终端要重新执行。

把仓库放到源码目录。开发机上的 `feat/tutorial-checkout` 若还没推到远端，就从开发机上传，不要克隆一个没有前端的旧分支：

```bash
# 在开发机执行，替换成服务器地址
rsync -a --delete \
  --exclude .git --exclude target --exclude node_modules \
  --exclude .runtime --exclude .nuxt --exclude .output --exclude dist \
  --exclude .env \
  ./ root@124.221.135.64:/usr/local/src/bz-store/
```

源码已经在 Git 远端时，可以在服务器执行 `git clone --branch feat/tutorial-checkout https://github.com/backkom-jz/bz-store.git /usr/local/src/bz-store`。

确认源码到位后再构建：

```bash
export PATH="/opt/node/bin:$HOME/.cargo/bin:$PATH"
cd /usr/local/src/bz-store
test -f package.json
test -f Cargo.lock
npm ci
npm run build --workspace @devstore/web
npm run build --workspace @devstore/admin
cargo build --locked --release --package devstore-server --bins
```

`test -f package.json` 失败就说明目录仍是空的发布根，不要继续。

产物对应关系：

| 构建产物 | 发布目录中的位置 |
| --- | --- |
| `target/release/devstore-server` | `bin/devstore-server` |
| `target/release/admin` | `bin/admin` |
| `apps/web/.output/` | `web/.output/` |
| `apps/admin/dist/` | `admin-dist/` |

Nuxt 的 `.output` 已包含运行依赖，不要再拷贝源码 `node_modules`。不要拷贝 `.env`、`.runtime`、`target` 其余文件、测试凭证或收款账户。

确认二进制能在 Rocky 10 上加载。此时还在源码目录，文件在 `target/release/`，不是 `bin/`。`bin/devstore-server` 要等装进发布目录之后才存在：

```bash
cd /usr/local/src/bz-store
test -x target/release/devstore-server
test -x target/release/admin
test -d apps/web/.output/server
test -f apps/admin/dist/index.html
file target/release/devstore-server
objdump -T target/release/devstore-server | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1
```

`file` 应显示 `ELF 64-bit LSB`、`x86-64`。最后一列 glibc 版本不得高于 `2.39`。四个 `test` 任一失败，就先回到上面的 `npm` 和 `cargo` 构建，不要打包。

在这台服务器上编译时，直接装进发布目录，不必再打归档上传：

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

记下输出的 `RELEASE_ID`，从下一节的权限命令继续。`ROOT` 仍指向这个版本。

在另一台机器编译时，再打成归档上传到服务器的 `/tmp`。不要把归档留在 Web 可访问目录：

```bash
cd /usr/local/src/bz-store
RELEASE_ID=$(date -u +%Y%m%dT%H%M%SZ)
STAGE=$(mktemp -d)
install -d "$STAGE/bin" "$STAGE/web" "$STAGE/admin-dist"
cp -a target/release/devstore-server target/release/admin "$STAGE/bin/"
cp -a apps/web/.output "$STAGE/web/.output"
cp -a apps/admin/dist/. "$STAGE/admin-dist/"
tar -C "$STAGE" -czf "/tmp/devstore-${RELEASE_ID}.tar.gz" bin web admin-dist
rm -rf "$STAGE"
```

## 4. 安装一个发布版本

在本机 `/usr/local/src/bz-store` 编译时，没有 `/tmp/devstore-*.tar.gz`。不要执行 `tar -xzf`。把源码目录里的产物复制进发布目录；`RELEASE_ID` 用你已经创建的那个，或重新生成一个：

```bash
cd /usr/local/src/bz-store
RELEASE_ID=20261010T092654Z
ROOT="/var/www/html/bz-store/releases/${RELEASE_ID}"
install -d -m 0755 "$ROOT/bin" "$ROOT/web" "$ROOT/admin-dist"
cp -a target/release/devstore-server target/release/admin "$ROOT/bin/"
cp -a apps/web/.output "$ROOT/web/.output"
cp -a apps/admin/dist/. "$ROOT/admin-dist/"
test -x "$ROOT/bin/devstore-server"
test -f "$ROOT/admin-dist/index.html"
```

两个 `test` 都没有输出后再做下面的权限。`cp` 报找不到文件，就先回到第 3 节确认 `target/release/devstore-server` 和 `apps/admin/dist/index.html` 存在。

只有构建发生在另一台机器、并且 `/tmp/devstore-${RELEASE_ID}.tar.gz` 已经上传时，才解压。归档顶层必须是 `bin`、`web`、`admin-dist`：

```bash
test -f "/tmp/devstore-${RELEASE_ID}.tar.gz"
tar -xzf "/tmp/devstore-${RELEASE_ID}.tar.gz" -C "/var/www/html/bz-store/releases/${RELEASE_ID}"
rm -f "/tmp/devstore-${RELEASE_ID}.tar.gz"
```

`test -f` 失败就不要运行 `tar`。若解压后多套了一层目录，先移平再继续。

权限要同时满足两件事：`devstore` 能执行 API，`nginx` 能沿路径读到后台页面。

```bash
ROOT="/var/www/html/bz-store/releases/${RELEASE_ID}"
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
```

两条 `test` 都必须成功，且没有输出。然后给 SELinux 标记 Web 可读类型：

```bash
semanage fcontext -a -t httpd_sys_content_t '/var/www/html/bz-store/releases(/.*)?'
restorecon -Rv /var/www/html/bz-store/releases
```

`semanage` 重复执行报已存在时，跳过添加，只执行 `restorecon`。

原子切换当前版本：

```bash
ln -sfn "/var/www/html/bz-store/releases/${RELEASE_ID}" /var/www/html/bz-store/current.new
mv -Tf /var/www/html/bz-store/current.new /var/www/html/bz-store/current
readlink -f /var/www/html/bz-store/current
```

`readlink` 必须指向刚安装的 `RELEASE_ID`。发布目录里不能有 `.env`。

## 5. 写入配置

两个文件都由 root 所有，组为 `devstore`，模式 `0640`。不要提交到 Git。

生成两枚不同的会话密钥，直接写入 `api.env`，不在终端显示内容。模板里的「替换成第一枚随机值」只有 27 字节，原样留下会被拒绝。每枚至少 32 字节，且两枚不能相同：

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

`/etc/devstore/api.env`：

```ini
APP_ENV=test
APP_PORT=18080
SITE_URL=http://124.221.135.64
DATABASE_URL=mysql://TEST_USER:URL编码后的密码@RDS_HOST:3306/bz_ztore_test?ssl-mode=VERIFY_IDENTITY&ssl-ca=/etc/devstore/rds-ca.pem
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

`SITE_URL` 不能带路径、查询或尾部斜杠。`APP_ENV=test` 时，`DATABASE_URL` 解析出的路径必须正好是 `/bz_ztore_test`，不能是 `/bz_ztore`、`/bz_store_test` 或带尾部斜杠的 `/bz_ztore_test/`。协议必须是 `mysql`。密码里的 `@`、`/`、`?`、`#`、`%` 要分别写成 `%40`、`%2F`、`%3F`、`%23`、`%25`，否则主机会被拆开，进程报「DATABASE_URL 必须指向 DevStore 对应的 MySQL 数据库」。不要写入 `MYSQL_TEST_DATABASE_URL`，API 不读这个变量。

SMTP 要么全部不写，要么同时写齐 `SMTP_HOST`、`EMAIL_FROM`、`EMAIL_OUTBOX_ENCRYPTION_KEY`，并按服务商要求成对填写 `SMTP_USERNAME` 和 `SMTP_PASSWORD`。只写其中一部分会让 API 无法启动。本阶段保持全部不写。

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

收紧权限：

```bash
chown root:devstore /etc/devstore/api.env /etc/devstore/web.env
chmod 0640 /etc/devstore/api.env /etc/devstore/web.env
```

确认 `api.env` 里有这些键名，命令只打印键，不打印值：

```bash
awk -F= '/^[[:space:]]*(export[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*=/ {
  key=$1; sub(/^[[:space:]]*export[[:space:]]+/, "", key); print key
}' /etc/devstore/api.env
```

至少要有 `APP_ENV`、`APP_PORT`、`SITE_URL`、`DATABASE_URL`、`REDIS_URL`、`JWT_SECRET`、`CSRF_SECRET`、`S3_ENDPOINT`、`S3_ACCESS_KEY`、`S3_SECRET_KEY`、`S3_BUCKET`。阿里云还要有 `S3_REGION=oss-cn-shanghai` 和 `S3_VIRTUAL_HOSTED=true`。每行写成 `KEY=value`，不要在 `=` 两侧加空格。`DATABASE_URL` 里的 `&` 必须留在同一行，不要用 shell 的 `source` 去加载这个文件。

`systemd-run --property=EnvironmentFile=` 不会把这个文件交给一次性命令。管理命令改走下面的启动程序，由它读取 `api.env`：

```bash
install -d /usr/local/libexec
cat > /usr/local/libexec/devstore-admin << 'EOF'
#!/usr/bin/env python3
import os, sys
env = os.environ.copy()
with open("/etc/devstore/api.env", encoding="utf-8") as handle:
    for raw in handle:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line[len("export "):].lstrip()
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        env[key.strip()] = value
os.chdir("/var/www/html/bz-store/current")
os.execvpe("./bin/admin", ["./bin/admin", *sys.argv[1:]], env)
EOF
chown root:devstore /usr/local/libexec/devstore-admin
chmod 0750 /usr/local/libexec/devstore-admin
```

## 6. 安装并启动服务

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

API 启动时会连接 MySQL 并执行迁移。对象存储桶还不存在时，进程可以启动，但 `ready` 返回 503。先启动 API，再初始化桶：

```bash
systemctl daemon-reload
systemctl enable --now devstore-api
systemctl is-active devstore-api
```

若状态不是 `active`，查看 `journalctl -u devstore-api -n 50 --no-pager`。配置错误会直接写出原因，例如库名不对、密钥太短或 SMTP 不完整。依赖连不上时，先修 RDS、CA 和 Redis，再 `systemctl restart devstore-api`。

初始化测试桶。这条命令可重复执行：

```bash
systemd-run --uid=devstore --gid=devstore --pipe --wait \
  /usr/local/libexec/devstore-admin init-storage
```

期望输出 `Private storage initialized`。若输出是 `缺少配置 CLAMAV_ADDR`，说明正在运行的二进制还是移除 ClamAV 之前的版本。不要把 `CLAMAV_ADDR` 写进 `api.env`，也不要安装 ClamAV。从开发机重新 rsync 当前源码，只重新 `cargo build --locked --release --package devstore-server --bins`，把新的两个二进制放进新的 `RELEASE_ID`，前端目录可以从上一版复制，然后切换 `current` 并 `systemctl restart devstore-api`。源码确认方式：`grep CLAMAV server/src/config.rs` 没有输出。

然后：

```bash
curl -fsS http://127.0.0.1:18080/api/v1/health/live
curl -fsS http://127.0.0.1:18080/api/v1/health/ready
systemctl enable --now devstore-web
curl -fsS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:3000/
```

`live` 的 `data.status` 为 `alive`。`ready` 的 `data.status` 为 `ready`。Nuxt 返回 `200`。`ready` 同时检查 MySQL、Redis 和对象存储列举；503 时不要继续配置 Nginx。

确认监听地址：

```bash
ss -lnt | awk 'NR==1 || /:80|:3000|:18080|:6379/'
```

3000 和 6379 必须在 `127.0.0.1`。18080 会出现在 `0.0.0.0`，这是程序的绑定方式；它仍然不能出现在防火墙或安全组的放行规则里。

## 7. 加载 Nginx

写入 `/etc/nginx/conf.d/store.conf`。这是临时 HTTP 配置。`/admin/` 的回退使用命名 location，避免 `alias` 和 `try_files` 组合找不到页面。

```nginx
server {
    listen 80;
    server_name 124.221.135.64;
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
nginx -T | grep -n 'server_name 124.221.135.64'
systemctl enable --now nginx
systemctl reload nginx
```

`grep` 只应出现一处。从服务器本机验收：

```bash
curl -fsS -o /dev/null -w 'home %{http_code}\n' http://127.0.0.1/ -H 'Host: 124.221.135.64'
curl -fsS -o /dev/null -w 'admin %{http_code}\n' http://127.0.0.1/admin/ -H 'Host: 124.221.135.64'
curl -fsS -o /dev/null -w 'admin-deep %{http_code}\n' http://127.0.0.1/admin/products/new -H 'Host: 124.221.135.64'
curl -fsS http://127.0.0.1/api/v1/health/ready -H 'Host: 124.221.135.64'
```

四个结果都应为成功：页面 `200`，`ready` 的状态为 `ready`。后台深链接必须返回 HTML，而不是 Nginx 404。然后从验收者网络访问 `http://124.221.135.64/`，确认安全组来源限制仍然有效。

## 8. 初始化管理员和测试数据

没有默认管理员密码。密码长度 12–128，只从标准输入传入：

```bash
printf '%s\n' '替换成一次性管理员密码' | systemd-run --uid=devstore --gid=devstore --pipe --wait \
  /usr/local/libexec/devstore-admin create --email admin@example.com --password-stdin
```

期望输出 `Admin created:` 和邮箱。重复创建同一邮箱会失败，不要为了重试去改生产库。

示例教程只允许出现在测试库。需要演示目录时才执行：

```bash
systemd-run --uid=devstore --gid=devstore --pipe --wait \
  /usr/local/libexec/devstore-admin seed-demo --admin-email admin@example.com
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
  /usr/local/libexec/devstore-admin payment-config \
  --admin-email admin@example.com --file /etc/devstore/payment.json
```

未写入收款配置时，下单返回「尚未配置收款信息，暂时不能购买」。这是预期行为，不是故障。

管理员 MFA 在预发布可以稍后做。正式切生产之前必须完成，因为生产进程缺少 `MFA_ENCRYPTION_KEY` 会拒绝启动。步骤见 [邮箱与管理员 MFA](auth-email-mfa.md)。在本机把其中的 `docker compose exec … admin` 换成 `systemd-run --uid=devstore --gid=devstore --pipe --wait /usr/local/libexec/devstore-admin`，后面仍接 `mfa-begin` 或 `mfa-confirm`。验证器种子和恢复码只在终端显示一次，不要送进网页、工单或聊天。

## 9. 验收记录

用测试账号完成下面各项，并把结果记在变更单里。不要上传真实付款凭证。

| 项目 | 通过标准 |
| --- | --- |
| 首页 | `http://124.221.135.64/` 返回店铺页面 |
| 后台入口 | `/admin/` 和 `/admin/products/new` 返回后台页面，刷新不落到 Nginx 404 |
| 健康检查 | `/api/v1/health/ready` 为 ready |
| 注册 | 未配置 SMTP 时注册失败并说明邮件不可用。不要改用生产邮件账号来“补测” |
| 管理员登录 | 使用第 8 步创建的账号进入后台 |
| 示例商品 | 若执行了 `seed-demo`，能看到示例教程，页面标明演示内容 |
| 未配置收款 | 不调用 `payment-config` 时，购买被拒绝 |
| 测试收款 | 配置虚构账户后，只用测试订单走下单、上传一张自备 PNG、驳回、重提、审核 |
| 凭证 | 只接受 PNG/JPEG，最大 5 MiB。当前不扫描恶意软件，审核时在后台查看，不打开图片里的链接 |
| 隔离 | 另一测试账号不能阅读未购买章节 |

付款凭证的处理边界见 [上传文件](uploads.md)。

## 10. 更新与回滚

每次更新都使用新的 `RELEASE_ID`。先按 [MySQL 备份](mysql-backups.md) 备份 `bz_ztore_test` 并确认能看到备份文件，再上传新目录。不要覆盖正在使用的发布目录。

```bash
ln -sfn "/var/www/html/bz-store/releases/${RELEASE_ID}" /var/www/html/bz-store/current.new
mv -Tf /var/www/html/bz-store/current.new /var/www/html/bz-store/current
systemctl restart devstore-api
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
| API 反复重启，日志是 `database migration failed` | 先 `systemctl stop devstore-api`。MySQL 的建表语句不能回滚；`_sqlx_migrations.success=0` 表示这次迁移只执行了一部分，继续重启不会重跑。空的 `bz_ztore_test` 可以删掉已建的表后，用 `mysql` 客户端重放 `server/migrations/0001_core.sql` 看真正的 SQL 错误 |
| `ready` 为 503 | MySQL、Redis，或测试桶尚未 `init-storage` |
| Nginx 502 | `devstore-web` 或 `devstore-api` 没在 3000 / 18080 监听；SELinux 是否拦截反代 |
| `/admin/products/new` 为 404 | `current/admin-dist/index.html` 是否存在，`nginx` 用户能否读取，命名 location 是否仍在配置里 |
| 注册不可用 | 本阶段未配置 SMTP，这是预期结果 |

## 11. 切到 store.bengzi.cn

下面全部完成后，才把预发布换成生产。缺一项就保持第 7 步的 IP 配置。

1. `store.bengzi.cn` 的 A 记录指向 `124.221.135.64`，证书是该域名的有效 `fullchain.pem` 和 `privkey.pem`。
2. 新建空库 `bz_ztore` 和只属于它的账号。不要把 `bz_ztore_test` 改名当成生产库，也不要把测试密钥拷进生产。
3. 备份目标库，并按 [MySQL 备份](mysql-backups.md) 做一次隔离恢复演练。对象存储使用独立的生产桶和独立密钥。
4. 更换 `JWT_SECRET`、`CSRF_SECRET`。另生成 `MFA_ENCRYPTION_KEY`（`openssl rand -base64 32`，解码后必须是 32 字节）。生产不写这枚密钥时，API 拒绝启动。
5. 写齐 SMTP，否则注册和找回密码仍然不可用。邮件密钥不要复用 JWT、CSRF 或 MFA 密钥。
6. 把 `APP_ENV` 改为 `production`，`SITE_URL` 改为 `https://store.bengzi.cn`，`DATABASE_URL` 的库名改为 `bz_ztore`，`S3_ENDPOINT` 保持 `https://`。其他值不满足时进程不会进入生产模式。
7. Nginx 改为该域名的 443，并把 80 重定向到 HTTPS。保留本文的反代、后台回退、6 MiB 限制和 `/verify-email`、`/reset-password` 不记录访问日志。
8. 重新执行第 6 步的健康检查和第 9 步的验收。然后创建生产管理员并完成 MFA。不要在生产库执行 `seed-demo`。

完成第 8 步之前，安全组不要对公网放开 80/443。
