-- 只在 bz_ztore_test 执行。可重复执行，已有同 slug 的记录不会再插入。
-- 先确认库名和管理员，结果里必须看到 bz_ztore_test，并且 admin_id 不是 NULL。
SELECT DATABASE() AS db_name;
SET @admin := (SELECT id FROM users WHERE email = 'backkom_jz@163.com' AND status = 1 LIMIT 1);
SELECT BIN_TO_UUID(@admin, 0) AS admin_id;

INSERT INTO categories (id, name, slug)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-0000000000c1', 0), 'Rust 开发', 'rust'
FROM DUAL
WHERE @admin IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM categories WHERE slug = 'rust');
SET @category := (SELECT id FROM categories WHERE slug = 'rust' LIMIT 1);

INSERT INTO products (id, title, slug, type, price_cents, description, status, is_demo, category_id)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000001', 0),
       'Rust + Axum：从接口到交易闭环',
       'rust-axum-starter',
       'tutorial',
       9900,
       '示例教程：围绕实际项目，学习类型安全的 API、数据库事务和部署。此内容用于本地演示。',
       'published',
       1,
       @category
FROM DUAL
WHERE @admin IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM products WHERE slug = 'rust-axum-starter');
SET @tutorial_product := (SELECT id FROM products WHERE slug = 'rust-axum-starter' LIMIT 1);

INSERT INTO tutorials (id, product_id, title, slug, description, status)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000002', 0),
       @tutorial_product,
       'Rust + Axum：从接口到交易闭环',
       'rust-axum-starter',
       '示例教程，包含免费预览和购买后的项目实践。',
       'published'
FROM DUAL
WHERE @tutorial_product IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM tutorials WHERE product_id = @tutorial_product);
SET @tutorial := (SELECT id FROM tutorials WHERE product_id = @tutorial_product LIMIT 1);

INSERT INTO tutorial_chapters (id, tutorial_id, title, content_md, is_preview, sort_order, status)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000003', 0),
       @tutorial,
       '01 · 从一个清晰的接口开始',
       '# 从一个清晰的接口开始\n\n一个值得维护的 API，应当让调用者理解成功与失败。先约定输入、输出和错误，再编写路由。\n\n```rust\nasync fn health() -> &''static str {\n    \"ready\"\n}\n```\n\n这是免费预览章节。后续章节用事务将订单与权益连接起来。',
       1,
       0,
       'published'
FROM DUAL
WHERE @tutorial IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM tutorial_chapters WHERE tutorial_id = @tutorial);

INSERT INTO tutorial_chapters (id, tutorial_id, title, content_md, is_preview, sort_order, status)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000004', 0),
       @tutorial,
       '02 · 用事务完成一次授权',
       '# 用事务完成一次授权\n\n付款审核涉及订单、凭证、权益和审计。这些写入要么一起成功，要么一起回滚。\n\n```rust\nlet mut tx = pool.begin().await?;\n// 锁定订单，核对状态与实际到账金额。\n// 更新凭证，授予权益，写入审计。\ntx.commit().await?;\n```\n\n对同一订单重复审核，应该返回当前结果。唯一约束是最后一道保护，不能只依赖前端禁用按钮。\n\n## 下一步\n\n尝试并发发起两次审核，并确认只产生一个权益。',
       0,
       1,
       'published'
FROM DUAL
WHERE @tutorial IS NOT NULL
  AND (SELECT COUNT(*) FROM tutorial_chapters WHERE tutorial_id = @tutorial) = 1;

INSERT INTO products (id, title, slug, type, price_cents, description, status, is_demo, category_id)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000011', 0),
       'Rust 速查笔记',
       'demo-rust-notes',
       'digital',
       0,
       '示例数字商品：零元领取后可以下载一份 PDF。此内容用于本地演示。',
       'published',
       1,
       @category
FROM DUAL
WHERE @admin IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM products WHERE slug = 'demo-rust-notes');
SET @notes := (SELECT id FROM products WHERE slug = 'demo-rust-notes' LIMIT 1);

INSERT INTO products (id, title, slug, type, price_cents, description, status, is_demo, category_id)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000021', 0),
       '架构评审服务',
       'demo-architecture-review',
       'service',
       0,
       '示例服务：领取后会打开一张工单。此内容用于本地演示。',
       'published',
       1,
       @category
FROM DUAL
WHERE @admin IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM products WHERE slug = 'demo-architecture-review');
SET @review := (SELECT id FROM products WHERE slug = 'demo-architecture-review' LIMIT 1);

INSERT INTO products (id, title, slug, type, price_cents, description, status, is_demo, category_id)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000031', 0),
       '已退款的示例文件',
       'demo-retired-sample',
       'digital',
       0,
       '示例：对应订单已退款，再次下载会被拒绝。此内容用于本地演示。',
       'published',
       1,
       @category
FROM DUAL
WHERE @admin IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM products WHERE slug = 'demo-retired-sample');
SET @retired := (SELECT id FROM products WHERE slug = 'demo-retired-sample' LIMIT 1);

INSERT INTO file_objects (id, owner_id, purpose, storage_key, content_type, size_bytes, sha256, status)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000012', 0),
       @admin,
       'digital_asset',
       'digital/a1000000-0000-4000-8000-000000000012',
       'application/pdf',
       302,
       'e8ffc01e6bbb36d10a81c6364c5a251ab0e3f150a7e35480f0aa440c9fc304aa',
       'clean'
FROM DUAL
WHERE @notes IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM product_files WHERE product_id = @notes);

INSERT INTO product_files (id, product_id, file_id)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000013', 0),
       @notes,
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000012', 0)
FROM DUAL
WHERE @notes IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM product_files WHERE product_id = @notes);

INSERT INTO file_objects (id, owner_id, purpose, storage_key, content_type, size_bytes, sha256, status)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000032', 0),
       @admin,
       'digital_asset',
       'digital/a1000000-0000-4000-8000-000000000032',
       'application/pdf',
       302,
       'e8ffc01e6bbb36d10a81c6364c5a251ab0e3f150a7e35480f0aa440c9fc304aa',
       'clean'
FROM DUAL
WHERE @retired IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM product_files WHERE product_id = @retired);

INSERT INTO product_files (id, product_id, file_id)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000033', 0),
       @retired,
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000032', 0)
FROM DUAL
WHERE @retired IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM product_files WHERE product_id = @retired);

INSERT INTO orders (id, order_no, user_id, amount_cents, status, expires_at, paid_at)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000014', 0),
       'DSA1000000000040008000000000000014',
       @admin,
       0,
       'completed',
       DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR),
       UTC_TIMESTAMP(6)
FROM DUAL
WHERE @notes IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM order_items i JOIN orders o ON o.id = i.order_id
    WHERE i.product_id = @notes AND o.user_id = @admin
  );
INSERT INTO order_items (id, order_id, product_id, product_title, product_type, unit_price_cents, quantity, subtotal_cents)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000015', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000014', 0),
       @notes,
       'Rust 速查笔记',
       'digital',
       0,
       1,
       0
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM order_items WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000015', 0))
  AND EXISTS (SELECT 1 FROM orders WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000014', 0));
INSERT INTO user_entitlements (id, user_id, order_item_id, product_id, status)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000016', 0),
       @admin,
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000015', 0),
       @notes,
       'active'
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM user_entitlements WHERE order_item_id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000015', 0));

INSERT INTO orders (id, order_no, user_id, amount_cents, status, expires_at, paid_at)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000022', 0),
       'DSA1000000000040008000000000000022',
       @admin,
       0,
       'fulfilling',
       DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR),
       UTC_TIMESTAMP(6)
FROM DUAL
WHERE @review IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM order_items i JOIN orders o ON o.id = i.order_id
    WHERE i.product_id = @review AND o.user_id = @admin
  );
INSERT INTO order_items (id, order_id, product_id, product_title, product_type, unit_price_cents, quantity, subtotal_cents)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000023', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000022', 0),
       @review,
       '架构评审服务',
       'service',
       0,
       1,
       0
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM order_items WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000023', 0))
  AND EXISTS (SELECT 1 FROM orders WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000022', 0));
INSERT INTO service_tickets (id, order_id, order_item_id, user_id, status)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000024', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000022', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000023', 0),
       @admin,
       'open'
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM service_tickets WHERE order_id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000022', 0))
  AND EXISTS (SELECT 1 FROM order_items WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000023', 0));
INSERT INTO ticket_messages (id, ticket_id, author_id, body)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000025', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000024', 0),
       @admin,
       '服务已开通，请在这里沟通交付内容。'
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM ticket_messages WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000025', 0))
  AND EXISTS (SELECT 1 FROM service_tickets WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000024', 0));
INSERT INTO ticket_messages (id, ticket_id, author_id, body)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000026', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000024', 0),
       @admin,
       '示例回复：交付说明可以写在这条工单里。'
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM ticket_messages WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000026', 0))
  AND EXISTS (SELECT 1 FROM service_tickets WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000024', 0));

INSERT INTO orders (id, order_no, user_id, amount_cents, status, expires_at, paid_at)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000034', 0),
       'DSA1000000000040008000000000000034',
       @admin,
       0,
       'refunded',
       DATE_ADD(UTC_TIMESTAMP(6), INTERVAL 24 HOUR),
       UTC_TIMESTAMP(6)
FROM DUAL
WHERE @retired IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM order_items i JOIN orders o ON o.id = i.order_id
    WHERE i.product_id = @retired AND o.user_id = @admin
  );
INSERT INTO order_items (id, order_id, product_id, product_title, product_type, unit_price_cents, quantity, subtotal_cents)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000035', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000034', 0),
       @retired,
       '已退款的示例文件',
       'digital',
       0,
       1,
       0
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM order_items WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000035', 0))
  AND EXISTS (SELECT 1 FROM orders WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000034', 0));
INSERT INTO user_entitlements (id, user_id, order_item_id, product_id, status, revoked_at)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000036', 0),
       @admin,
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000035', 0),
       @retired,
       'revoked',
       UTC_TIMESTAMP(6)
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM user_entitlements WHERE order_item_id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000035', 0));
INSERT INTO refunds (id, order_id, amount_cents, reference_no, note, actor_id)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000037', 0),
       UUID_TO_BIN('a1000000-0000-4000-8000-000000000034', 0),
       0,
       'DEMO-REFUND-0001',
       '示例退款，用于核对权益已经收回。',
       @admin
FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM refunds WHERE reference_no = 'DEMO-REFUND-0001')
  AND EXISTS (SELECT 1 FROM orders WHERE id = UUID_TO_BIN('a1000000-0000-4000-8000-000000000034', 0));

INSERT INTO blog_posts (id, title, slug, excerpt, content_md, status, published_at)
SELECT UUID_TO_BIN('a1000000-0000-4000-8000-000000000041', 0),
       '数字商品怎么交付',
       'shipping-digital-goods',
       '示例文章：文件不公开，每次下载都核对权益。',
       '# 数字商品怎么交付\n\n买家付款后得到的是账号里的权益，不是一个长期有效的公开链接。\n\n每次下载都由服务端再核对一次。退款登记完成后，下一次下载会失败。\n\n这篇文章只用于本地演示，不进入商品目录。',
       'published',
       UTC_TIMESTAMP(6)
FROM DUAL
WHERE @admin IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM blog_posts WHERE slug = 'shipping-digital-goods');

SELECT slug, type, status FROM products WHERE slug IN ('rust-axum-starter', 'demo-rust-notes', 'demo-architecture-review', 'demo-retired-sample');
SELECT slug, status FROM blog_posts WHERE slug = 'shipping-digital-goods';
SELECT order_no, status FROM orders WHERE user_id = @admin AND order_no LIKE 'DSA1000000%';
