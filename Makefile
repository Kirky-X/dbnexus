.PHONY: help test-all test-sqlite test-postgres test-mysql docker-up docker-down docker-logs clippy-all

help: ## 显示帮助信息
	@echo "DBNexus 测试命令"
	@echo ""
	@echo "数据库相关:"
	@echo "  make docker-up       - 启动 PostgreSQL 和 MySQL 容器"
	@echo "  make docker-down     - 停止数据库容器"
	@echo "  make docker-logs     - 查看数据库日志"
	@echo ""
	@echo "测试相关:"
	@echo "  make test-sqlite     - 运行 SQLite 测试"
	@echo "  make test-postgres   - 运行 PostgreSQL 测试"
	@echo "  make test-mysql      - 运行 MySQL 测试"
	@echo "  make test-all        - 运行所有数据库测试"
	@echo ""
	@echo "代码质量:"
	@echo "  make clippy-sqlite   - 运行 SQLite 的 clippy 检查"
	@echo "  make clippy-postgres - 运行 PostgreSQL 的 clippy 检查"
	@echo "  make clippy-mysql    - 运行 MySQL 的 clippy 检查"
	@echo "  make clippy-all      - 运行所有数据库的 clippy 检查"

# 数据库相关
docker-up: ## 启动数据库容器
	@echo "启动数据库容器..."
	docker compose up -d
	@echo "等待数据库就绪..."
	@sleep 15
	@./scripts/test-databases.sh

docker-down: ## 停止数据库容器
	docker compose down

docker-logs: ## 查看数据库日志
	docker compose logs -f

# 测试相关
test-sqlite: ## 运行 SQLite 测试
	@echo "运行 SQLite 测试..."
	TEST_DB_TYPE=sqlite cargo test --features sqlite

test-postgres: ## 运行 PostgreSQL 测试
	@echo "运行 PostgreSQL 测试..."
	TEST_DB_TYPE=postgres DATABASE_URL=postgres://dbnexus:dbnexus_password@localhost:15432/dbnexus_test cargo test --features postgres

test-mysql: ## 运行 MySQL 测试
	@echo "运行 MySQL 测试..."
	TEST_DB_TYPE=mysql DATABASE_URL=mysql://dbnexus:dbnexus_password@localhost:13306/dbnexus_test cargo test --features mysql

test-all: ## 运行所有数据库测试
	@echo "运行所有数据库测试..."
	@make test-sqlite
	@make test-postgres
	@make test-mysql

# Clippy 检查
clippy-sqlite: ## 运行 SQLite 的 clippy 检查
	@echo "运行 SQLite clippy 检查..."
	cargo clippy --features "sqlite,all-optional"

clippy-postgres: ## 运行 PostgreSQL 的 clippy 检查
	@echo "运行 PostgreSQL clippy 检查..."
	cargo clippy --features "postgres,all-optional"

clippy-mysql: ## 运行 MySQL 的 clippy 检查
	@echo "运行 MySQL clippy 检查..."
	cargo clippy --features "mysql,all-optional"

clippy-all: ## 运行所有数据库的 clippy 检查
	@echo "运行所有数据库的 clippy 检查..."
	@make clippy-sqlite
	@make clippy-postgres
	@make clippy-mysql