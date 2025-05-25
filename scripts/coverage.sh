#!/usr/bin/env bash
# Code coverage script for open-with project
# Generates code coverage report using cargo-tarpaulin

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${YELLOW}📊 Generating code coverage report...${NC}"
echo ""

# Check if cargo-tarpaulin is installed
if ! command -v cargo-tarpaulin &>/dev/null; then
  echo -e "${RED}❌ cargo-tarpaulin is not installed${NC}"
  echo -e "${YELLOW}Installing cargo-tarpaulin...${NC}"
  cargo install cargo-tarpaulin
fi

# Clean previous coverage data
echo -e "${YELLOW}🧹 Cleaning previous coverage data...${NC}"
rm -f cobertura.xml tarpaulin-report.html tarpaulin-report.json

# Run tests with coverage
echo -e "${YELLOW}🧪 Running tests with coverage...${NC}"
if cargo tarpaulin \
  --verbose \
  --all-features \
  --workspace \
  --timeout 120 \
  --out Xml \
  --out Html \
  --out Json \
  --output-dir . \
  --exclude-files "*/tests/*" \
  --exclude-files "*/build.rs" \
  --ignore-panics \
  --ignore-tests; then
  echo -e "${GREEN}✅ Coverage report generated successfully${NC}"
else
  echo -e "${RED}❌ Failed to generate coverage report${NC}"
  exit 1
fi

# Parse and display coverage summary
if [ -f "tarpaulin-report.json" ]; then
  echo ""
  echo -e "${BLUE}📈 Coverage Summary:${NC}"

  # Extract coverage percentage using jq if available
  if command -v jq &>/dev/null; then
    COVERAGE=$(jq -r '.coverage' tarpaulin-report.json 2>/dev/null || echo "N/A")
    echo -e "Total Coverage: ${GREEN}${COVERAGE}%${NC}"

    # Show per-file coverage
    echo ""
    echo -e "${YELLOW}Per-file coverage:${NC}"
    jq -r '.files | to_entries | .[] | "\(.key): \(.value.coverage)%"' tarpaulin-report.json 2>/dev/null | sort
  else
    echo -e "${YELLOW}Install jq for detailed coverage summary${NC}"
  fi
fi

echo ""
echo -e "${GREEN}📄 Coverage reports generated:${NC}"
echo -e "  - ${BLUE}cobertura.xml${NC} (XML format for CI/CD)"
echo -e "  - ${BLUE}tarpaulin-report.html${NC} (HTML report)"
echo -e "  - ${BLUE}tarpaulin-report.json${NC} (JSON format)"

# Open HTML report if possible
if [ -f "tarpaulin-report.html" ]; then
  echo ""
  echo -e "${YELLOW}Opening HTML report in browser...${NC}"

  # Try to open the HTML report
  if command -v xdg-open &>/dev/null; then
    xdg-open tarpaulin-report.html 2>/dev/null || true
  elif command -v open &>/dev/null; then
    open tarpaulin-report.html 2>/dev/null || true
  else
    echo -e "${YELLOW}View the HTML report at: file://$(pwd)/tarpaulin-report.html${NC}"
  fi
fi

# Check coverage threshold
THRESHOLD=70
if command -v jq &>/dev/null && [ -f "tarpaulin-report.json" ]; then
  COVERAGE=$(jq -r '.coverage' tarpaulin-report.json 2>/dev/null || echo "0")
  # Remove % if present and convert to integer
  COVERAGE_INT=$(echo "$COVERAGE" | sed 's/%//g' | cut -d. -f1)

  if [ "$COVERAGE_INT" -lt "$THRESHOLD" ]; then
    echo ""
    echo -e "${RED}⚠️  Warning: Coverage ${COVERAGE}% is below threshold of ${THRESHOLD}%${NC}"
  else
    echo ""
    echo -e "${GREEN}✅ Coverage ${COVERAGE}% meets threshold of ${THRESHOLD}%${NC}"
  fi
fi
