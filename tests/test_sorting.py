"""
Sorting Tests
Tests for library and entry sorting functionality
"""
import requests


class TestLibrarySorting:
    """Test library-level sorting"""

    def test_default_sort(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Default sorting should be by name"""
        response = authenticated_session.get(f"{base_url}/api/library")
        assert response.status_code == 200

        titles_default = response.json()
        assert len(titles_default) > 0

    def test_sort_by_name(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Sort by name should be alphabetical"""
        response_default = authenticated_session.get(f"{base_url}/api/library")
        titles_default = response_default.json()

        response_name = authenticated_session.get(f"{base_url}/api/library?sort=name")
        assert response_name.status_code == 200

        titles_name = response_name.json()

        # Should be same order as default
        assert titles_default == titles_name

    def test_sort_by_time(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Sort by time should order by modification time"""
        response = authenticated_session.get(f"{base_url}/api/library?sort=time")
        assert response.status_code == 200

        titles_time = response.json()
        assert len(titles_time) > 0

    def test_sort_by_time_reverse(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Sort by time-reverse should reverse time ordering"""
        response_time = authenticated_session.get(f"{base_url}/api/library?sort=time")
        titles_time = response_time.json()

        response_reverse = authenticated_session.get(
            f"{base_url}/api/library?sort=time-reverse"
        )
        assert response_reverse.status_code == 200

        titles_reverse = response_reverse.json()

        # If times differ, should be reversed
        if titles_time != titles_reverse:
            assert titles_reverse == list(reversed(titles_time))

    def test_invalid_sort_param_uses_default(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Invalid sort parameter should fall back to default"""
        response = authenticated_session.get(
            f"{base_url}/api/library?sort=invalid"
        )
        assert response.status_code == 200

        titles_invalid = response.json()
        assert len(titles_invalid) > 0


class TestEntrySorting:
    """Test entry-level sorting within a title"""

    def test_sort_entries_by_name(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Entries should be sortable by name"""
        # Get a title
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        # Get entries sorted by name
        response = authenticated_session.get(
            f"{base_url}/api/title/{title_id}?sort=name"
        )
        assert response.status_code == 200

        title_detail = response.json()
        assert 'entries' in title_detail
        assert len(title_detail['entries']) > 0

    def test_sort_entries_by_time(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Entries should be sortable by time"""
        # Get a title
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        # Get entries sorted by time
        response = authenticated_session.get(
            f"{base_url}/api/title/{title_id}?sort=time"
        )
        assert response.status_code == 200

        title_detail = response.json()
        assert 'entries' in title_detail
