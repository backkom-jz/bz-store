CREATE TABLE users (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), email VARCHAR(255) NOT NULL UNIQUE,
 password_hash TEXT NOT NULL, display_name VARCHAR(80) NOT NULL DEFAULT '',
 status SMALLINT NOT NULL DEFAULT 1 CHECK(status IN (0,1)),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK(email=lower(trim(email)))
);
CREATE TABLE roles (id UUID PRIMARY KEY DEFAULT gen_random_uuid(),code TEXT NOT NULL UNIQUE CHECK(code IN ('user','admin')),name TEXT NOT NULL);
INSERT INTO roles(code,name) VALUES('user','用户'),('admin','管理员');
CREATE TABLE user_roles (user_id UUID NOT NULL REFERENCES users(id),role_id UUID NOT NULL REFERENCES roles(id),PRIMARY KEY(user_id,role_id));
CREATE TABLE user_sessions (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),user_id UUID NOT NULL REFERENCES users(id),
 token_hash CHAR(64) NOT NULL UNIQUE,csrf_hash CHAR(64) NOT NULL,expires_at TIMESTAMPTZ NOT NULL,revoked_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user ON user_sessions(user_id);
CREATE TABLE categories (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),parent_id UUID REFERENCES categories(id),name VARCHAR(80) NOT NULL,slug VARCHAR(220) NOT NULL UNIQUE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK(slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$')
);
CREATE TABLE products (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),category_id UUID REFERENCES categories(id),
 type VARCHAR(20) NOT NULL CHECK(type IN ('digital','tutorial','service')),
 title VARCHAR(200) NOT NULL,slug VARCHAR(220) NOT NULL UNIQUE,description TEXT NOT NULL DEFAULT '',
 price_cents BIGINT NOT NULL CHECK(price_cents BETWEEN 0 AND 100000000),currency CHAR(3) NOT NULL DEFAULT 'CNY' CHECK(currency='CNY'),
 status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published','archived')),is_demo BOOLEAN NOT NULL DEFAULT false,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK(slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),CHECK(status<>'published' OR type='tutorial')
);
CREATE INDEX products_published ON products(status,created_at DESC,id DESC);
CREATE TABLE tutorials (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),product_id UUID NOT NULL UNIQUE REFERENCES products(id),
 title VARCHAR(200) NOT NULL,slug VARCHAR(220) NOT NULL UNIQUE,description TEXT NOT NULL DEFAULT '',
 status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK(slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$')
);
CREATE TABLE tutorial_chapters (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),tutorial_id UUID NOT NULL REFERENCES tutorials(id),
 title VARCHAR(200) NOT NULL,content_md TEXT NOT NULL DEFAULT '',is_preview BOOLEAN NOT NULL DEFAULT false,
 sort_order INTEGER NOT NULL DEFAULT 0 CHECK(sort_order>=0),status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX chapters_tutorial ON tutorial_chapters(tutorial_id,sort_order,id);
CREATE TABLE orders (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),order_no VARCHAR(40) NOT NULL UNIQUE,user_id UUID NOT NULL REFERENCES users(id),
 amount_cents BIGINT NOT NULL CHECK(amount_cents BETWEEN 0 AND 100000000),currency CHAR(3) NOT NULL DEFAULT 'CNY' CHECK(currency='CNY'),
 status VARCHAR(30) NOT NULL DEFAULT 'pending_payment' CHECK(status IN ('pending_payment','payment_review','paid','fulfilling','completed','cancelled','refunded')),
 payment_instructions JSONB NOT NULL DEFAULT '{}',expires_at TIMESTAMPTZ NOT NULL DEFAULT now()+interval '24 hours',paid_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX orders_user_created ON orders(user_id,created_at DESC,id DESC);
CREATE INDEX orders_status_created ON orders(status,expires_at,created_at DESC);
CREATE TABLE order_items (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),order_id UUID NOT NULL UNIQUE REFERENCES orders(id),product_id UUID NOT NULL REFERENCES products(id),
 product_title VARCHAR(200) NOT NULL,product_type VARCHAR(20) NOT NULL CHECK(product_type IN ('digital','tutorial','service')),
 unit_price_cents BIGINT NOT NULL CHECK(unit_price_cents BETWEEN 0 AND 100000000),quantity INTEGER NOT NULL CHECK(quantity=1),
 subtotal_cents BIGINT NOT NULL CHECK(subtotal_cents=unit_price_cents*quantity),currency CHAR(3) NOT NULL DEFAULT 'CNY' CHECK(currency='CNY'),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE order_requests (
 user_id UUID NOT NULL REFERENCES users(id),idempotency_key VARCHAR(128) NOT NULL,request_fingerprint CHAR(64) NOT NULL,
 order_id UUID REFERENCES orders(id),created_at TIMESTAMPTZ NOT NULL DEFAULT now(),PRIMARY KEY(user_id,idempotency_key)
);
CREATE TABLE file_objects (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),owner_id UUID NOT NULL REFERENCES users(id),purpose TEXT NOT NULL CHECK(purpose='payment_proof'),
 storage_key TEXT NOT NULL UNIQUE,content_type TEXT NOT NULL CHECK(content_type IN ('image/png','image/jpeg')),
 size_bytes BIGINT NOT NULL CHECK(size_bytes BETWEEN 1 AND 5242880),sha256 CHAR(64) NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('quarantined','clean','rejected','deleting')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE payment_submissions (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),order_id UUID NOT NULL REFERENCES orders(id),file_id UUID NOT NULL UNIQUE REFERENCES file_objects(id),
 status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','approved','rejected')),
 reviewed_by UUID REFERENCES users(id),reviewed_at TIMESTAMPTZ,review_note TEXT,
 actual_amount_cents BIGINT CHECK(actual_amount_cents BETWEEN 0 AND 100000000),payment_channel TEXT,transaction_reference VARCHAR(128),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK((status='pending' AND reviewed_by IS NULL AND reviewed_at IS NULL) OR (status<>'pending' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL)),
 CHECK(status<>'approved' OR (actual_amount_cents IS NOT NULL AND payment_channel IS NOT NULL AND transaction_reference IS NOT NULL AND length(transaction_reference)>0)),
 CHECK(status<>'rejected' OR length(trim(review_note))>0)
);
CREATE UNIQUE INDEX payment_one_pending ON payment_submissions(order_id) WHERE status='pending';
CREATE UNIQUE INDEX payment_one_approved ON payment_submissions(order_id) WHERE status='approved';
CREATE UNIQUE INDEX payment_reference_unique ON payment_submissions(payment_channel,transaction_reference) WHERE status='approved';
CREATE INDEX payments_status_created ON payment_submissions(status,created_at,id);
CREATE TABLE user_entitlements (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),user_id UUID NOT NULL REFERENCES users(id),order_item_id UUID NOT NULL UNIQUE REFERENCES order_items(id),
 product_id UUID NOT NULL REFERENCES products(id),status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK(status IN ('active','revoked')),
 granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),revoked_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 CHECK((status='active' AND revoked_at IS NULL) OR (status='revoked' AND revoked_at IS NOT NULL))
);
CREATE INDEX entitlements_user_product ON user_entitlements(user_id,product_id,status);
CREATE TABLE admin_audit_logs (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(),actor_id UUID REFERENCES users(id),action TEXT NOT NULL,target_id UUID,
 detail JSONB NOT NULL DEFAULT '{}',created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_created ON admin_audit_logs(created_at DESC,id DESC);
CREATE TABLE site_settings(key TEXT PRIMARY KEY,value_json JSONB NOT NULL,created_at TIMESTAMPTZ NOT NULL DEFAULT now(),updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE FUNCTION touch_updated_at() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.updated_at=now(); RETURN NEW; END $$;
DO $$ DECLARE t TEXT; BEGIN
 FOREACH t IN ARRAY ARRAY['users','user_sessions','categories','products','tutorials','tutorial_chapters','orders','order_items','file_objects','payment_submissions','user_entitlements','site_settings'] LOOP
  EXECUTE format('CREATE TRIGGER touch_updated_at BEFORE UPDATE ON %I FOR EACH ROW EXECUTE FUNCTION touch_updated_at()',t);
 END LOOP;
END $$;
