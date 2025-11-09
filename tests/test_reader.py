"""
Reader and Progress Tests
Tests for manga reader and reading progress tracking
"""
import pytest
import requests


class TestReader:
    """Test reader page functionality"""

    def test_reader_page_loads(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Reader page should load successfully"""
        # Get title and entry IDs
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        title_detail = response.json()
        entry_id = title_detail['entries'][0]['id']

        # Access reader
        response = authenticated_session.get(
            f"{base_url}/reader/{title_id}/{entry_id}/1"
        )
        assert response.status_code == 200
        assert 'reader' in response.text.lower()


class TestProgress:
    """Test progress saving and loading"""

    def test_save_progress(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Progress save endpoint should accept page numbers"""
        # Get title and entry IDs
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        title_detail = response.json()
        entry_id = title_detail['entries'][0]['id']

        # Save progress
        test_page = 15
        response = authenticated_session.post(
            f"{base_url}/api/progress/{title_id}/{entry_id}",
            json={"page": test_page}
        )
        assert response.status_code == 200

    def test_load_progress(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Progress load endpoint should return saved progress"""
        # Get title and entry IDs
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        title_detail = response.json()
        entry_id = title_detail['entries'][0]['id']

        # Save progress first
        test_page = 20
        authenticated_session.post(
            f"{base_url}/api/progress/{title_id}/{entry_id}",
            json={"page": test_page}
        )

        # Load progress
        response = authenticated_session.get(
            f"{base_url}/api/progress/{title_id}/{entry_id}"
        )
        assert response.status_code == 200

        progress = response.json()
        assert progress.get('page') == test_page

    def test_get_all_progress(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """All progress endpoint should return dictionary of progress"""
        response = authenticated_session.get(f"{base_url}/api/progress")
        assert response.status_code == 200

        all_progress = response.json()
        assert isinstance(all_progress, dict)

    def test_update_progress(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Updating progress should overwrite previous value"""
        # Get title and entry IDs
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()
        title_id = titles[0]['id']

        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        title_detail = response.json()
        entry_id = title_detail['entries'][0]['id']

        # Save initial progress
        authenticated_session.post(
            f"{base_url}/api/progress/{title_id}/{entry_id}",
            json={"page": 10}
        )

        # Update progress
        new_page = 25
        authenticated_session.post(
            f"{base_url}/api/progress/{title_id}/{entry_id}",
            json={"page": new_page}
        )

        # Verify update
        response = authenticated_session.get(
            f"{base_url}/api/progress/{title_id}/{entry_id}"
        )
        progress = response.json()
        assert progress.get('page') == new_page

    def test_progress_persists_across_entries(
        self,
        authenticated_session: requests.Session,
        base_url: str
    ) -> None:
        """Progress should be independent for different entries"""
        # Get title with multiple entries
        response = authenticated_session.get(f"{base_url}/api/library")
        titles = response.json()

        # Find a title with multiple entries
        title_id = None
        for title in titles:
            if title['entries'] >= 2:
                title_id = title['id']
                break

        if title_id is None:
            pytest.skip("No title with multiple entries found")

        response = authenticated_session.get(f"{base_url}/api/title/{title_id}")
        title_detail = response.json()
        entries = title_detail['entries'][:2]  # Get first two entries

        # Save different progress for each entry
        authenticated_session.post(
            f"{base_url}/api/progress/{title_id}/{entries[0]['id']}",
            json={"page": 10}
        )
        authenticated_session.post(
            f"{base_url}/api/progress/{title_id}/{entries[1]['id']}",
            json={"page": 20}
        )

        # Verify they're independent
        response1 = authenticated_session.get(
            f"{base_url}/api/progress/{title_id}/{entries[0]['id']}"
        )
        response2 = authenticated_session.get(
            f"{base_url}/api/progress/{title_id}/{entries[1]['id']}"
        )

        assert response1.json().get('page') == 10
        assert response2.json().get('page') == 20
