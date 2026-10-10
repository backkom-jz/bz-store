ALTER TABLE user_sessions ADD COLUMN mfa_verified_at DATETIME(6) NULL;
ALTER TABLE user_sessions ADD COLUMN mfa_method VARCHAR(16) NULL CHECK(mfa_method IS NULL OR mfa_method IN ('totp','recovery'));
CREATE TABLE admin_mfa (
 user_id BINARY(16) PRIMARY KEY,encrypted_secret BLOB NOT NULL,
 status VARCHAR(16) NOT NULL CHECK(status IN ('pending','enabled')),
 last_counter BIGINT NULL,enabled_at DATETIME(6),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 CHECK((status='pending' AND enabled_at IS NULL) OR (status='enabled' AND enabled_at IS NOT NULL)),
 FOREIGN KEY(user_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE TABLE admin_recovery_codes (
 id BINARY(16) PRIMARY KEY,user_id BINARY(16) NOT NULL,code_hash CHAR(64) NOT NULL,
 consumed_at DATETIME(6),created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 UNIQUE(user_id,code_hash),FOREIGN KEY(user_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE INDEX admin_recovery_user ON admin_recovery_codes(user_id,consumed_at);
CREATE TRIGGER touch_admin_mfa BEFORE UPDATE ON admin_mfa FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);
