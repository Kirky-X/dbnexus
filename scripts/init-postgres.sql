-- PostgreSQL 初始化脚本
-- 创建测试表

CREATE TABLE IF NOT EXISTS test_users (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS test_accounts (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    balance DECIMAL(10, 2) DEFAULT 0.00,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY (user_id) REFERENCES test_users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS test_orders (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER,
    amount DECIMAL(10, 2) NOT NULL,
    status VARCHAR(50) DEFAULT 'pending',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, FOREIGN KEY (user_id) REFERENCES test_users(id) ON DELETE SET NULL
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_test_users_email ON test_users(email);
CREATE INDEX IF NOT EXISTS idx_test_accounts_user_id ON test_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_test_orders_user_id ON test_orders(user_id);
CREATE INDEX IF NOT EXISTS idx_test_orders_status ON test_orders(status);

-- 插入测试数据
INSERT  INTO test_users (name, email) VALUES
    ('Alice', 'alice@example.com'),
    ('Bob', 'bob@example.com'),
    ('Charlie', 'charlie@example.com')ON CONFLICT (email) DO NOTHING;

INSERT  INTO test_accounts (user_id, balance) VALUES
    (1, 1000.00),
    (2, 500.00),
    (3, 750.00)ON CONFLICT DO NOTHING;

INSERT  INTO test_orders (user_id, amount, status) VALUES
    (1, 100.00, 'completed'),
    (2, 50.00, 'pending'),
    (3, 75.00, 'completed');