"""
Library API Tests
Tests for library scanning, statistics, and API endpoints
"""
import requests
from typing import Dict, Any


class TestLibraryAPI:
    """Test library API endpoints"""

    def test_get_stats(
        self,
        authenticated_session: requests.Session,
        base_url: str,
        test_data: Dict[str, Any]
    ) -> None:
        """Library statistics should return correct counts"""
        response = authenticated_session.get(f"{base_url}/api/stats")
        assert response.status_code == 200

        stats = response.json()
        assert 'titles' in stats
        assert 'entries' in stats
        assert 'pages' in stats

        # Verify expected counts
        assert stats['titles'] == test_data['expected_titles_count']
        assert stats['entries'] == test_data['expected_entries_count']
        assert stats['pages'] == test_data['expected_pages_count']

    def test_get_library(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Library endpoint should return list of titles"""
        response = authenticated_session.get(f"{base_url}/api/library")
        assert response.status_code == 200

        titles = response.json()
        assert isinstance(titles, list)
        assert len(titles) > 0

        # Verify title structure
        first_title = titles[0]
        assert 'id' in first_title
        assert 'title' in first_title
        assert 'entries' in first_title
        assert 'pages' in first_title

    def test_get_title_detail(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Title detail endpoint should return entries"""
        # Get a title ID
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        # Get title details
        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        assert response.status_code == 200

        title_detail = response.json()
        assert 'id' in title_detail
        assert 'title' in title_detail
        assert 'entries' in title_detail
        assert isinstance(title_detail['entries'], list)
        assert len(title_detail['entries']) > 0

        # Verify entry structure
        first_entry = title_detail['entries'][0]
        assert 'id' in first_entry
        assert 'title' in first_entry
        assert 'pages' in first_entry

    def test_get_page_image(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Page endpoint should return image data"""
        # Get title and entry IDs
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        title_detail = response.json()
        entry_id = title_detail['entries'][0]['id']

        # Get first page
        response = authenticated_session.get(
            f"{base_url}/api/page/{title_id}/{entry_id}/1"
        )
        assert response.status_code == 200
        assert len(response.content) > 0
        assert 'image/' in response.headers.get('content-type', '')

    def test_invalid_page_returns_404(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Requesting invalid page number should return 404"""
        # Get title and entry IDs
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        title_detail = response.json()
        entry_id = title_detail['entries'][0]['id']

        # Request invalid page
        response = authenticated_session.get(
            f"{base_url}/api/page/{title_id}/{entry_id}/99999"
        )
        assert response.status_code == 404
