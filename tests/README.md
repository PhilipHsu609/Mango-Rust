# Mango-Rust Test Suite

Comprehensive integration tests for all core functionality.

## Quick Start

```bash
# Ensure server is running
cargo run --release

# Run all tests
pytest tests/ -v
```

## Structure

```
tests/
├── conftest.py          # Shared fixtures and configuration
├── test_auth.py         # Authentication and sessions (5 tests)
├── test_library.py      # Library API endpoints (5 tests)
├── test_reader.py       # Reader and progress (6 tests)
├── test_sorting.py      # Sorting functionality (7 tests)
└── requirements.txt     # Python dependencies
```

## Setup

### Prerequisites

```bash
conda activate mango
pip install -r tests/requirements.txt
```

### Admin Credentials

Tests use `admin/admin`. If needed, reset the password:

```bash
python3 -c "import bcrypt; print(bcrypt.hashpw(b'admin', bcrypt.gensalt()).decode())"
sqlite3 ~/mango/mango.db "UPDATE users SET password = '<hash>' WHERE username = 'admin';"
```

## Running Tests

```bash
# All tests
pytest tests/ -v

# Specific file
pytest tests/test_auth.py -v

# Specific test
pytest tests/test_auth.py::TestAuthentication::test_login_with_valid_credentials -v

# Pattern matching
pytest tests/ -k "progress" -v

# Stop on first failure
pytest tests/ -x

# Show slowest tests
pytest tests/ --durations=5
```

## Test Coverage (23 tests, 100% passing)

### Authentication (5 tests)
- Redirect when not authenticated
- Reject invalid credentials
- Accept valid credentials
- Session persistence
- Logout functionality

### Library API (5 tests)
- Library statistics
- Title listing
- Title details with entries
- Page image serving
- 404 for invalid pages

### Reader & Progress (6 tests)
- Reader page loading
- Save/load progress
- Get all progress
- Update progress
- Independent progress per entry

### Sorting (7 tests)
- Default, name, time, reverse sorting
- Invalid parameter fallback
- Entry-level sorting

## Fixtures

- `base_url`: Server URL (http://localhost:9000)
- `session`: Unauthenticated HTTP session
- `authenticated_session`: Authenticated session with valid cookies
- `test_data`: Expected counts (3 titles, 30 entries, 4717 pages)

## Troubleshooting

**Connection Refused**: Ensure server is running with `cargo run --release`

**Authentication Fails**: Reset admin password (see Setup section above)

**Wrong Test Counts**: Update `test_data` fixture in `conftest.py` to match your library

## Notes

- Tests expect specific library content (3 titles, 30 entries, 4717 pages)
- Some tests have sequential dependencies (acceptable for integration tests)
- Tests use the actual database (no isolation yet)
