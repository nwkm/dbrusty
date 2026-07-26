#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[seed]${NC} $1"; }
ok()    { echo -e "${GREEN}[seed]${NC} $1"; }
warn()  { echo -e "${YELLOW}[seed]${NC} $1"; }
err()   { echo -e "${RED}[seed]${NC} $1"; }

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Seed all docker-compose databases with fake data."
    echo ""
    echo "Options:"
    echo "  --skip-postgres    Skip PostgreSQL seeding"
    echo "  --skip-mysql       Skip MySQL seeding"
    echo "  --skip-mariadb     Skip MariaDB seeding"
    echo "  --skip-mongodb     Skip MongoDB seeding"
    echo "  --skip-redis       Skip Redis seeding"
    echo "  --skip-build       Skip cargo build (use existing binaries)"
    echo "  --help             Show this help message"
    exit 0
}

SKIP_POSTGRES=false
SKIP_MYSQL=false
SKIP_MARIADB=false
SKIP_MONGODB=false
SKIP_REDIS=false
SKIP_BUILD=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-postgres) SKIP_POSTGRES=true; shift ;;
        --skip-mysql)    SKIP_MYSQL=true;    shift ;;
        --skip-mariadb)  SKIP_MARIADB=true;  shift ;;
        --skip-mongodb)  SKIP_MONGODB=true;  shift ;;
        --skip-redis)    SKIP_REDIS=true;    shift ;;
        --skip-build)    SKIP_BUILD=true;     shift ;;
        --help)          usage ;;
        *) err "Unknown option: $1"; usage ;;
    esac
done

# ── Build seed binaries ──────────────────────────────────────────
if [[ "$SKIP_BUILD" == false ]]; then
    info "Building seed binaries..."
    cargo build --bin seed_postgresql --bin seed_mysql --bin seed_mariadb --bin seed_mongodb --bin seed_redis
    ok "Build complete"
fi

# ── Helper: wait for a TCP port ───────────────────────────────────
wait_for_port() {
    local host="$1" port="$2" label="$3" max=${4:-60}
    info "Waiting for $label (${host}:${port})..."
    for i in $(seq 1 "$max"); do
        if nc -z "$host" "$port" 2>/dev/null; then
            ok "$label is ready"
            return 0
        fi
        sleep 1
    done
    err "$label did not become ready within ${max}s"
    return 1
}

# ── Seed PostgreSQL ───────────────────────────────────────────────
seed_postgres() {
    wait_for_port "127.0.0.1" 5432 "PostgreSQL"
    info "Seeding PostgreSQL..."
    cargo run --bin seed_postgresql -- "postgres://postgres@127.0.0.1:5432/postgres"
    ok "PostgreSQL seeded"
}

# ── Seed MySQL ────────────────────────────────────────────────────
seed_mysql() {
    wait_for_port "127.0.0.1" 3307 "MySQL"
    info "Seeding MySQL..."
    cargo run --bin seed_mysql -- "mysql://root:password@127.0.0.1:3307/test"
    ok "MySQL seeded"
}

# ── Seed MariaDB ──────────────────────────────────────────────────
seed_mariadb() {
    wait_for_port "127.0.0.1" 3308 "MariaDB"
    info "Seeding MariaDB..."
    cargo run --bin seed_mariadb -- "mysql://root:password@127.0.0.1:3308/mariadb"
    ok "MariaDB seeded"
}

# ── Seed MongoDB ──────────────────────────────────────────────────
seed_mongodb() {
    wait_for_port "127.0.0.1" 27017 "MongoDB"
    info "Seeding MongoDB..."
    cargo run --bin seed_mongodb -- "mongodb://127.0.0.1:27017" "dbrusty_seed"
    ok "MongoDB seeded"
}

# ── Seed Redis ────────────────────────────────────────────────────
seed_redis() {
    wait_for_port "127.0.0.1" 6379 "Redis"
    info "Seeding Redis across databases 0-5 ..."
    cargo run --bin seed_redis -- "redis://127.0.0.1:6379"
    ok "Redis seeded"
}

# ── Main ──────────────────────────────────────────────────────────
echo ""
echo -e "${CYAN}══════════════════════════════════════════${NC}"
echo -e "${CYAN}  dbrusty — Database Seeder             ${NC}"
echo -e "${CYAN}══════════════════════════════════════════${NC}"
echo ""

if [[ "$SKIP_POSTGRES" == false ]]; then
    seed_postgres
fi

if [[ "$SKIP_MYSQL" == false ]]; then
    seed_mysql
fi

if [[ "$SKIP_MARIADB" == false ]]; then
    seed_mariadb
fi

if [[ "$SKIP_MONGODB" == false ]]; then
    seed_mongodb
fi

if [[ "$SKIP_REDIS" == false ]]; then
    seed_redis
fi

echo ""
echo -e "${GREEN}══════════════════════════════════════════${NC}"
echo -e "${GREEN}  All seeding complete!                  ${NC}"
echo -e "${GREEN}══════════════════════════════════════════${NC}"
echo ""
echo "Connection strings for the app:"
echo "  PostgreSQL:  postgres://postgres@127.0.0.1:5432/postgres"
echo "  MySQL:       mysql://root:password@127.0.0.1:3307/test"
echo "  MariaDB:     mysql://root:password@127.0.0.1:3308/mariadb"
echo "  MongoDB:     mongodb://127.0.0.1:27017 (db: dbrusty_seed)"
echo "  Redis:       redis://127.0.0.1:6379/0 (databases 0-16, db0 default)"
echo "               data seeded in db0 (strings), db1 (hashes), db2 (lists),"
echo "               db3 (sets), db4 (sorted sets), db5 (streams)"
