#!/usr/bin/env bash
# Verification script for open-with project
# Run this before committing to ensure code quality

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🔍 Running verification checks...${NC}"
echo ""

# 1. Format check
echo -e "${YELLOW}📝 Checking code formatting...${NC}"
if cargo fmt -- --check; then
  echo -e "${GREEN}✅ Code formatting is correct${NC}"
else
  echo -e "${RED}❌ Code needs formatting${NC}"
  echo "Run 'cargo fmt' to fix formatting issues"
  exit 1
fi
echo ""

# 2. Clippy
echo -e "${YELLOW}🔎 Running clippy...${NC}"
if cargo clippy -- -D warnings -D clippy::all -D clippy::pedantic -A clippy::module_name_repetitions -A clippy::struct_excessive_bools -A clippy::unnecessary-debug-formatting; then
  echo -e "${GREEN}✅ Clippy checks passed${NC}"
else
  echo -e "${RED}❌ Clippy found issues${NC}"
  exit 1
fi
echo ""

# 3. Tests
echo -e "${YELLOW}🧪 Running tests...${NC}"
if cargo test; then
  echo -e "${GREEN}✅ All tests passed${NC}"
else
  echo -e "${RED}❌ Tests failed${NC}"
  exit 1
fi
echo ""

# 4. Documentation
echo -e "${YELLOW}📚 Checking documentation...${NC}"
if RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --document-private-items --quiet; then
  echo -e "${GREEN}✅ Documentation is valid${NC}"
else
  echo -e "${RED}❌ Documentation has issues${NC}"
  exit 1
fi
echo ""

# 5. Security audit
echo -e "${YELLOW}🔒 Running security audit...${NC}"
if cargo audit --deny warnings; then
  echo -e "${GREEN}✅ No security vulnerabilities found${NC}"
else
  echo -e "${RED}❌ Security vulnerabilities detected${NC}"
  exit 1
fi
echo ""

# 6. License check
echo -e "${YELLOW}📜 Checking licenses...${NC}"
if cargo deny check licenses; then
  echo -e "${GREEN}✅ License compliance verified${NC}"
else
  echo -e "${RED}❌ License issues found${NC}"
  exit 1
fi
echo ""

# 7. Build check
echo -e "${YELLOW}🔨 Checking build...${NC}"
if cargo build --release; then
  echo -e "${GREEN}✅ Release build successful${NC}"
else
  echo -e "${RED}❌ Build failed${NC}"
  exit 1
fi
echo ""

# Optional: Run coverage if requested
if [[ "$1" == "--with-coverage" ]]; then
  echo -e "${YELLOW}📊 Running code coverage...${NC}"
  if ./scripts/coverage.sh; then
    echo -e "${GREEN}✅ Coverage report generated${NC}"
  else
    echo -e "${RED}❌ Coverage generation failed${NC}"
    exit 1
  fi
  echo ""
fi

echo -e "${GREEN}🎉 All verification checks passed!${NC}"
echo -e "${YELLOW}You can now commit your changes.${NC}"
