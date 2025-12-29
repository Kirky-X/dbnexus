#!/bin/bash
# 数据库测试脚本

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}DBNexus 数据库测试脚本${NC}"
echo -e "${GREEN}========================================${NC}"

# 检查 Docker 是否运行
if ! docker info > /dev/null 2>&1; then
    echo -e "${RED}错误: Docker 未运行，请先启动 Docker${NC}"
    exit 1
fi

# 启动数据库容器
echo -e "${YELLOW}启动数据库容器...${NC}"
docker compose up -d

# 等待数据库就绪
echo -e "${YELLOW}等待数据库就绪...${NC}"
sleep 15

# 检查数据库连接
echo -e "${YELLOW}检查数据库连接...${NC}"

# 检查 PostgreSQL
if docker exec dbnexus-postgres pg_isready -U dbnexus > /dev/null 2>&1; then
    echo -e "${GREEN}✓ PostgreSQL 就绪${NC}"
else
    echo -e "${RED}✗ PostgreSQL 未就绪${NC}"
fi

# 检查 MySQL
if docker exec dbnexus-mysql mysqladmin ping -h localhost -u dbnexus -pdbnexus_password > /dev/null 2>&1; then
    echo -e "${GREEN}✓ MySQL 就绪${NC}"
else
    echo -e "${RED}✗ MySQL 未就绪${NC}"
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}数据库连接信息:${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "PostgreSQL: postgres://dbnexus:dbnexus_password@localhost:15432/dbnexus_test"
echo -e "MySQL:      mysql://dbnexus:dbnexus_password@localhost:13306/dbnexus_test"
echo -e "SQLite:     sqlite::memory:"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "${YELLOW}运行测试命令:${NC}"
echo -e "cargo test --features sqlite"
echo -e "cargo test --features postgres"
echo -e "cargo test --features mysql"
echo ""
echo -e "${YELLOW}停止数据库: docker compose down${NC}"
echo -e "${YELLOW}查看日志: docker compose logs -f${NC}"