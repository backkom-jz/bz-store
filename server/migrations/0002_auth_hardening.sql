ALTER TABLE users ADD COLUMN email_verified_at DATETIME(6) NULL;
UPDATE users SET email_verified_at=created_at WHERE email_verified_at IS NULL;
CREATE TABLE auth_tokens (
 id BINARY(16) PRIMARY KEY,user_id BINARY(16) NOT NULL,
 purpose VARCHAR(32) NOT NULL CHECK(purpose IN ('email_verification','password_reset')),
 token_hash CHAR(64) NOT NULL UNIQUE,expires_at DATETIME(6) NOT NULL,consumed_at DATETIME(6),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 FOREIGN KEY(user_id) REFERENCES users(id)
) ENGINE=InnoDB;
CREATE INDEX auth_tokens_user ON auth_tokens(user_id,purpose,expires_at);
CREATE TABLE email_outbox (
 id BINARY(16) PRIMARY KEY,recipient VARCHAR(255) NOT NULL,kind VARCHAR(32) NOT NULL,
 encrypted_payload BLOB NOT NULL,status VARCHAR(16) NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','sending','sent','failed')),
 attempts SMALLINT NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 8),available_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
 locked_until DATETIME(6),sent_at DATETIME(6),last_error VARCHAR(64),
 created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB;
CREATE INDEX email_outbox_ready ON email_outbox(status,available_at,locked_until,created_at);
CREATE TRIGGER touch_email_outbox BEFORE UPDATE ON email_outbox FOR EACH ROW SET NEW.updated_at=CURRENT_TIMESTAMP(6);

