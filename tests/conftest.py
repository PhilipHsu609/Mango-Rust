"""
Pytest configuration and shared fixtures for Mango-Rust tests
"""
import pytest
import requests
from typing import Generator, Dict, Any

BASE_URL = "http://localhost:9000"


@pytest.fixture(scope="session")
def base_url() -> str:
    """Base URL for the Mango server"""
    return BASE_URL


@pytest.fixture(scope="function")
def session() -> Generator[requests.Session, None, None]:
    """Create a new session for each test"""
    s = requests.Session()
    yield s
    s.close()


@pytest.fixture(scope="function")
def authenticated_session(base_url: str) -> Generator[requests.Session, None, None]:
    """
    Create an authenticated session by logging in.
    Returns a session with valid authentication cookies.
    """
    s = requests.Session()

    # Perform login to get session cookie
    login_response = s.post(
        f"{base_url}/login",
        data={"username": "admin", "password": "admin"},
        headers={"Content-Type": "application/x-www-form-urlencoded"},
        allow_redirects=False  # Don't follow redirects to preserve cookies
    )

    # Verify login succeeded (should redirect)
    assert login_response.status_code in [302, 303], f"Login failed with status {login_response.status_code}"

    # Verify session cookie was set
    assert 'set-cookie' in login_response.headers or len(s.cookies) > 0, "No session cookie received"

    # Verify we can access protected pages
    home_response = s.get(f"{base_url}/", allow_redirects=False)
    assert home_response.status_code == 200, f"Cannot access protected page, got status {home_response.status_code}"

    yield s
    s.close()


@pytest.fixture(scope="session")
def test_data() -> Dict[str, Any]:
    """
    Test data and expected values
    These should match your actual test library
    """
    return {
        "expected_titles_count": 3,
        "expected_entries_count": 30,
        "expected_pages_count": 4717,
    }
