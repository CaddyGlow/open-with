#!/usr/bin/env bash
# Format script for openit project

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🎨 Formatting code...${NC}"

# Format Rust code
cargo fmt

# Run clippy with auto-fix where possible
echo -e "${YELLOW}🔧 Running clippy fixes...${NC}"
cargo clippy --fix --allow-dirty --allow-staged -- -D warnings

# Format again in case clippy made changes
cargo fmt

echo -e "${GREEN}✅ Code formatted!${NC}"
echo ""
echo -e "${YELLOW}Run './scripts/verify.sh' to verify all checks pass.${NC}"
