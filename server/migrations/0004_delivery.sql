SET @publish_check = (
  SELECT cc.CONSTRAINT_NAME
  FROM information_schema.CHECK_CONSTRAINTS cc
  JOIN information_schema.TABLE_CONSTRAINTS tc
    ON tc.CONSTRAINT_SCHEMA = cc.CONSTRAINT_SCHEMA
   AND tc.CONSTRAINT_NAME = cc.CONSTRAINT_NAME
  WHERE tc.TABLE_SCHEMA = DATABASE()
    AND tc.TABLE_NAME = 'products'
    AND cc.CHECK_CLAUSE LIKE '%published%'
    AND cc.CHECK_CLAUSE LIKE '%tutorial%'
  LIMIT 1
);
SET @sql = IF(@publish_check IS NULL, 'SELECT 1', CONCAT('ALTER TABLE products DROP CHECK `', @publish_check, '`'));
PREPARE drop_publish FROM @sql;
EXECUTE drop_publish;
DEALLOCATE PREPARE drop_publish;

SET @purpose_check = (
  SELECT cc.CONSTRAINT_NAME
  FROM information_schema.CHECK_CONSTRAINTS cc
  JOIN information_schema.TABLE_CONSTRAINTS tc
    ON tc.CONSTRAINT_SCHEMA = cc.CONSTRAINT_SCHEMA
   AND tc.CONSTRAINT_NAME = cc.CONSTRAINT_NAME
  WHERE tc.TABLE_SCHEMA = DATABASE()
    AND tc.TABLE_NAME = 'file_objects'
    AND cc.CHECK_CLAUSE LIKE '%payment_proof%'
  LIMIT 1
);
SET @sql = IF(@purpose_check IS NULL, 'SELECT 1', CONCAT('ALTER TABLE file_objects DROP CHECK `', @purpose_check, '`'));
PREPARE drop_purpose FROM @sql;
EXECUTE drop_purpose;
DEALLOCATE PREPARE drop_purpose;
ALTER TABLE file_objects ADD CONSTRAINT file_objects_purpose_chk CHECK (purpose IN ('payment_proof','digital_asset'));

SET @type_check = (
  SELECT cc.CONSTRAINT_NAME
  FROM information_schema.CHECK_CONSTRAINTS cc
  JOIN information_schema.TABLE_CONSTRAINTS tc
    ON tc.CONSTRAINT_SCHEMA = cc.CONSTRAINT_SCHEMA
   AND tc.CONSTRAINT_NAME = cc.CONSTRAINT_NAME
  WHERE tc.TABLE_SCHEMA = DATABASE()
    AND tc.TABLE_NAME = 'file_objects'
    AND cc.CHECK_CLAUSE LIKE '%image/png%'
  LIMIT 1
);
SET @sql = IF(@type_check IS NULL, 'SELECT 1', CONCAT('ALTER TABLE file_objects DROP CHECK `', @type_check, '`'));
PREPARE drop_type FROM @sql;
EXECUTE drop_type;
DEALLOCATE PREPARE drop_type;
ALTER TABLE file_objects ADD CONSTRAINT file_objects_content_type_chk CHECK (content_type IN ('image/png','image/jpeg','application/pdf','application/zip'));

CREATE TABLE product_files (
 id BINARY(16) PRIMARY KEY,
 product_id BINARY(16) NOT NULL UNIQUE,
 file_id BINARY(16) NOT NULL UNIQUE,
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(product_id) REFERENCES products(id),
 FOREIGN KEY(file_id) REFERENCES file_objects(id)
) ENGINE=InnoDB;

CREATE TABLE download_logs (
 id BINARY(16) PRIMARY KEY,
 user_id BINARY(16) NOT NULL,
 file_id BINARY(16) NOT NULL,
 entitlement_id BINARY(16) NOT NULL,
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(user_id) REFERENCES users(id),
 FOREIGN KEY(file_id) REFERENCES file_objects(id),
 FOREIGN KEY(entitlement_id) REFERENCES user_entitlements(id)
) ENGINE=InnoDB;
CREATE INDEX download_logs_user ON download_logs(user_id,created_at DESC,id DESC);

CREATE TABLE service_tickets (
 id BINARY(16) PRIMARY KEY,
 order_id BINARY(16) NOT NULL UNIQUE,
 order_item_id BINARY(16) NOT NULL UNIQUE,
 user_id BINARY(16) NOT NULL,
 status VARCHAR(20) NOT NULL DEFAULT 'open' CHECK(status IN ('open','completed','closed')),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 closed_at DATETIME(6),
 FOREIGN KEY(order_id) REFERENCES orders(id),
 FOREIGN KEY(order_item_id) REFERENCES order_items(id),
 FOREIGN KEY(user_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE INDEX service_tickets_user ON service_tickets(user_id,created_at DESC,id DESC);

CREATE TABLE ticket_messages (
 id BINARY(16) PRIMARY KEY,
 ticket_id BINARY(16) NOT NULL,
 author_id BINARY(16) NOT NULL,
 body TEXT NOT NULL,
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(ticket_id) REFERENCES service_tickets(id),
 FOREIGN KEY(author_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE INDEX ticket_messages_ticket ON ticket_messages(ticket_id,created_at,id);

CREATE TABLE refunds (
 id BINARY(16) PRIMARY KEY,
 order_id BINARY(16) NOT NULL UNIQUE,
 amount_cents BIGINT NOT NULL CHECK(amount_cents BETWEEN 0 AND 100000000),
 reference_no VARCHAR(128) NOT NULL UNIQUE,
 note TEXT NOT NULL,
 actor_id BINARY(16) NOT NULL,
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(order_id) REFERENCES orders(id),
 FOREIGN KEY(actor_id) REFERENCES users(id)
) ENGINE=InnoDB;

CREATE TABLE blog_posts (
 id BINARY(16) PRIMARY KEY,
 title VARCHAR(200) NOT NULL,
 slug VARCHAR(220) NOT NULL UNIQUE,
 excerpt TEXT NOT NULL DEFAULT (''),
 content_md MEDIUMTEXT NOT NULL,
 status VARCHAR(20) NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published')),
 published_at DATETIME(6),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK(slug REGEXP '^[a-z0-9]+(-[a-z0-9]+)*$')
) ENGINE=InnoDB;

CREATE TRIGGER touch_service_tickets BEFORE UPDATE ON service_tickets FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
CREATE TRIGGER touch_blog_posts BEFORE UPDATE ON blog_posts FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
