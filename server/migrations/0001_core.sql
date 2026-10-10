CREATE TABLE users (
 id BINARY(16) PRIMARY KEY, email VARCHAR(255) NOT NULL UNIQUE,
 password_hash TEXT NOT NULL, display_name VARCHAR(80) NOT NULL DEFAULT '',
 status SMALLINT NOT NULL DEFAULT 1 CHECK(status IN (0,1)),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6), updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK(email=lower(trim(email)))
) ENGINE=InnoDB;
CREATE TABLE roles (id BINARY(16) PRIMARY KEY,code VARCHAR(50) NOT NULL UNIQUE CHECK(code IN ('user','admin')),name VARCHAR(80) NOT NULL) ENGINE=InnoDB;
INSERT INTO roles(id,code,name) VALUES(UNHEX(REPLACE(UUID(),'-','')),'user','用户'),(UNHEX(REPLACE(UUID(),'-','')),'admin','管理员');
CREATE TABLE user_roles (user_id BINARY(16) NOT NULL,role_id BINARY(16) NOT NULL,PRIMARY KEY(user_id,role_id),FOREIGN KEY(user_id) REFERENCES users(id),FOREIGN KEY(role_id) REFERENCES roles(id)) ENGINE=InnoDB;
CREATE TABLE user_sessions (
 id BINARY(16) PRIMARY KEY,user_id BINARY(16) NOT NULL,
 token_hash CHAR(64) NOT NULL UNIQUE,csrf_hash CHAR(64) NOT NULL,expires_at DATETIME(6) NOT NULL,revoked_at DATETIME(6),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(user_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE INDEX sessions_user ON user_sessions(user_id);
CREATE TABLE categories (
 id BINARY(16) PRIMARY KEY,parent_id BINARY(16),name VARCHAR(80) NOT NULL,slug VARCHAR(220) NOT NULL UNIQUE,
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK(slug REGEXP '^[a-z0-9]+(-[a-z0-9]+)*$'),FOREIGN KEY(parent_id) REFERENCES categories(id)
) ENGINE=InnoDB;
CREATE TABLE products (
 id BINARY(16) PRIMARY KEY,category_id BINARY(16),
 type VARCHAR(20) NOT NULL CHECK(type IN ('digital','tutorial','service')),
 title VARCHAR(200) NOT NULL,slug VARCHAR(220) NOT NULL UNIQUE,description TEXT NOT NULL DEFAULT (''),
 price_cents BIGINT NOT NULL CHECK(price_cents BETWEEN 0 AND 100000000),currency CHAR(3) NOT NULL DEFAULT 'CNY' CHECK(currency='CNY'),
 status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published','archived')),is_demo BOOLEAN NOT NULL DEFAULT false,
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK(slug REGEXP '^[a-z0-9]+(-[a-z0-9]+)*$'),CHECK(status<>'published' OR type='tutorial'),FOREIGN KEY(category_id) REFERENCES categories(id)
) ENGINE=InnoDB;
CREATE INDEX products_published ON products(status,created_at DESC,id DESC);
CREATE TABLE tutorials (
 id BINARY(16) PRIMARY KEY,product_id BINARY(16) NOT NULL UNIQUE,
 title VARCHAR(200) NOT NULL,slug VARCHAR(220) NOT NULL UNIQUE,description TEXT NOT NULL DEFAULT (''),
 status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published')),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK(slug REGEXP '^[a-z0-9]+(-[a-z0-9]+)*$'),FOREIGN KEY(product_id) REFERENCES products(id)
) ENGINE=InnoDB;
CREATE TABLE tutorial_chapters (
 id BINARY(16) PRIMARY KEY,tutorial_id BINARY(16) NOT NULL,
 title VARCHAR(200) NOT NULL,content_md TEXT NOT NULL DEFAULT (''),is_preview BOOLEAN NOT NULL DEFAULT false,
 sort_order INTEGER NOT NULL DEFAULT 0 CHECK(sort_order>=0),status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published')),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(tutorial_id) REFERENCES tutorials(id)
) ENGINE=InnoDB;
CREATE INDEX chapters_tutorial ON tutorial_chapters(tutorial_id,sort_order,id);
CREATE TABLE orders (
 id BINARY(16) PRIMARY KEY,order_no VARCHAR(40) NOT NULL UNIQUE,user_id BINARY(16) NOT NULL,
 amount_cents BIGINT NOT NULL CHECK(amount_cents BETWEEN 0 AND 100000000),currency CHAR(3) NOT NULL DEFAULT 'CNY' CHECK(currency='CNY'),
 status VARCHAR(30) NOT NULL DEFAULT 'pending_payment' CHECK(status IN ('pending_payment','payment_review','paid','fulfilling','completed','cancelled','refunded')),
 payment_instructions JSON NOT NULL DEFAULT ('{}'),expires_at DATETIME(6) NOT NULL,paid_at DATETIME(6),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(user_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE INDEX orders_user_created ON orders(user_id,created_at DESC,id DESC);
CREATE INDEX orders_status_created ON orders(status,expires_at,created_at DESC);
CREATE TABLE order_items (
 id BINARY(16) PRIMARY KEY,order_id BINARY(16) NOT NULL UNIQUE,product_id BINARY(16) NOT NULL,
 product_title VARCHAR(200) NOT NULL,product_type VARCHAR(20) NOT NULL CHECK(product_type IN ('digital','tutorial','service')),
 unit_price_cents BIGINT NOT NULL CHECK(unit_price_cents BETWEEN 0 AND 100000000),quantity INTEGER NOT NULL CHECK(quantity=1),
 subtotal_cents BIGINT NOT NULL,currency CHAR(3) NOT NULL DEFAULT 'CNY' CHECK(currency='CNY'),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(order_id) REFERENCES orders(id),FOREIGN KEY(product_id) REFERENCES products(id),
 CHECK(subtotal_cents=unit_price_cents*quantity)
) ENGINE=InnoDB;
CREATE TABLE order_requests (
 user_id BINARY(16) NOT NULL,idempotency_key VARCHAR(128) NOT NULL,request_fingerprint CHAR(64) NOT NULL,
 order_id BINARY(16),created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),PRIMARY KEY(user_id,idempotency_key),
 FOREIGN KEY(user_id) REFERENCES users(id),FOREIGN KEY(order_id) REFERENCES orders(id)
) ENGINE=InnoDB;
CREATE TABLE file_objects (
 id BINARY(16) PRIMARY KEY,owner_id BINARY(16) NOT NULL,purpose VARCHAR(32) NOT NULL CHECK(purpose='payment_proof'),
 storage_key VARCHAR(512) NOT NULL UNIQUE,content_type VARCHAR(32) NOT NULL CHECK(content_type IN ('image/png','image/jpeg')),
 size_bytes BIGINT NOT NULL CHECK(size_bytes BETWEEN 1 AND 5242880),sha256 CHAR(64) NOT NULL,
 status VARCHAR(32) NOT NULL CHECK(status IN ('quarantined','clean','rejected','deleting')),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(owner_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE TABLE payment_submissions (
 id BINARY(16) PRIMARY KEY,order_id BINARY(16) NOT NULL,file_id BINARY(16) NOT NULL UNIQUE,
 status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','approved','rejected')),
 reviewed_by BINARY(16),reviewed_at DATETIME(6),review_note TEXT,
 actual_amount_cents BIGINT CHECK(actual_amount_cents BETWEEN 0 AND 100000000),payment_channel VARCHAR(64),transaction_reference VARCHAR(128),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK((status='pending' AND reviewed_by IS NULL AND reviewed_at IS NULL) OR (status<>'pending' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL)),
 CHECK(status<>'approved' OR (actual_amount_cents IS NOT NULL AND payment_channel IS NOT NULL AND transaction_reference IS NOT NULL AND length(transaction_reference)>0)),
 CHECK(status<>'rejected' OR (review_note IS NOT NULL AND length(trim(review_note))>0)),
 FOREIGN KEY(order_id) REFERENCES orders(id),FOREIGN KEY(file_id) REFERENCES file_objects(id),FOREIGN KEY(reviewed_by) REFERENCES users(id)
) ENGINE=InnoDB;
ALTER TABLE payment_submissions ADD COLUMN pending_order_id BINARY(16) GENERATED ALWAYS AS (CASE WHEN status='pending' THEN order_id ELSE NULL END) STORED;
ALTER TABLE payment_submissions ADD COLUMN approved_order_id BINARY(16) GENERATED ALWAYS AS (CASE WHEN status='approved' THEN order_id ELSE NULL END) STORED;
ALTER TABLE payment_submissions ADD COLUMN approved_payment_channel VARCHAR(255) GENERATED ALWAYS AS (CASE WHEN status='approved' THEN payment_channel ELSE NULL END) STORED;
ALTER TABLE payment_submissions ADD COLUMN approved_transaction_reference VARCHAR(128) GENERATED ALWAYS AS (CASE WHEN status='approved' THEN transaction_reference ELSE NULL END) STORED;
CREATE UNIQUE INDEX payment_one_pending ON payment_submissions(pending_order_id);
CREATE UNIQUE INDEX payment_one_approved ON payment_submissions(approved_order_id);
CREATE UNIQUE INDEX payment_reference_unique ON payment_submissions(approved_payment_channel,approved_transaction_reference);
CREATE INDEX payments_status_created ON payment_submissions(status,created_at,id);
CREATE TABLE user_entitlements (
 id BINARY(16) PRIMARY KEY,user_id BINARY(16) NOT NULL,order_item_id BINARY(16) NOT NULL UNIQUE,
 product_id BINARY(16) NOT NULL,status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK(status IN ('active','revoked')),
 granted_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),revoked_at DATETIME(6),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK((status='active' AND revoked_at IS NULL) OR (status='revoked' AND revoked_at IS NOT NULL)),
 FOREIGN KEY(user_id) REFERENCES users(id),FOREIGN KEY(order_item_id) REFERENCES order_items(id),FOREIGN KEY(product_id) REFERENCES products(id)
) ENGINE=InnoDB;
CREATE INDEX entitlements_user_product ON user_entitlements(user_id,product_id,status);
CREATE TABLE admin_audit_logs (
 id BINARY(16) PRIMARY KEY,actor_id BINARY(16),action VARCHAR(255) NOT NULL,target_id BINARY(16),
 detail JSON NOT NULL DEFAULT ('{}'),created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),FOREIGN KEY(actor_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE INDEX audit_created ON admin_audit_logs(created_at DESC,id DESC);
CREATE TABLE site_settings(setting_key VARCHAR(191) PRIMARY KEY,value_json JSON NOT NULL,created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)) ENGINE=InnoDB;
CREATE TRIGGER touch_users BEFORE UPDATE ON users FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_user_sessions BEFORE UPDATE ON user_sessions FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_categories BEFORE UPDATE ON categories FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_products BEFORE UPDATE ON products FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_tutorials BEFORE UPDATE ON tutorials FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_tutorial_chapters BEFORE UPDATE ON tutorial_chapters FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_orders BEFORE UPDATE ON orders FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_order_items BEFORE UPDATE ON order_items FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_file_objects BEFORE UPDATE ON file_objects FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_payment_submissions BEFORE UPDATE ON payment_submissions FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_user_entitlements BEFORE UPDATE ON user_entitlements FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_site_settings BEFORE UPDATE ON site_settings FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
