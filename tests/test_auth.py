"""
Authentication Tests
Tests for login, logout, and session management
"""
import requests


class TestAuthentication:
    """Test authentication and session management"""

    def test_redirect_to_login_when_not_authenticated(
        self,
        session: requests.Session,
        base_url: str
    ) -> None:
        """Unauthenticated requests should redirect to login page"""
        response = session.get(f"{base_url}/", allow_redirects=False)
        assert response.status_code == 303
        assert '/login' in response.headers.get('Location', '')

    def test_reject_invalid_credentials(
        self,
        session: requests.Session,
        base_url: str
    ) -> None:
        """Invalid credentials should be rejected and redirect back to login"""
        response = session.post(
            f"{base_url}/login",
            data={"username": "wrong", "password": "wrong"},
            headers={"Content-Type": "application/x-www-form-urlencoded"},
            allow_redirects=False
        )
        assert response.status_code == 303
        assert '/login' in response.headers.get('Location', '')

    def test_login_with_valid_credentials(
        self,
        session: requests.Session,
        base_url: str
    ) -> None:
        """Valid credentials should allow successful login"""
        response = session.post(
            f"{base_url}/login",
            data={"username": "admin", "password": "admin"},
            headers={"Content-Type": "application/x-www-form-urlencoded"},
            allow_redirects=True
        )
        assert response.status_code == 200

    def test_session_persists_after_login(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Authenticated session should work for subsequent requests"""
        response = authenticated_session.get(f"{base_url}/")
        assert response.status_code == 200
        assert 'Mango' in response.text

    def test_logout_clears_session(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Logout should clear the session and redirect to login"""
        # Verify we're logged in
        response = authenticated_session.get(f"{base_url}/")
        assert response.status_code == 200

        # Logout
        logout_response = authenticated_session.get(
            f"{base_url}/logout",
            allow_redirects=False
        )
        assert logout_response.status_code in [302, 303]

        # Try to access protected page - should redirect to login
        protected_response = authenticated_session.get(
            f"{base_url}/",
            allow_redirects=False
        )
        assert protected_response.status_code == 303
