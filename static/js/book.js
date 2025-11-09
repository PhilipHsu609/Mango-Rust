// Current modal state
let currentTitleId = '';
let currentEntryId = '';
let currentProgress = 0;
let currentPages = 0;

// Get URL parameters
const urlParams = new URLSearchParams(window.location.search);

// Handle sort dropdown change
const sortSelect = document.getElementById('sort-select');
if (sortSelect) {
    sortSelect.addEventListener('change', function(e) {
        const [sort, ascend] = e.target.value.split(':');
        const url = new URL(window.location.href);
        url.searchParams.set('sort', sort);
        url.searchParams.set('ascend', ascend);
        window.location.href = url.toString();
    });

    // Set selected option based on URL parameters
    const currentSort = urlParams.get('sort') || 'title';
    const currentAscend = urlParams.get('ascend') || '1';
    const currentValue = `${currentSort}:${currentAscend}`;

    // Try to find and select the matching option
    const options = sortSelect.options;
    for (let i = 0; i < options.length; i++) {
        if (options[i].value === currentValue) {
            sortSelect.selectedIndex = i;
            break;
        }
    }
}

// Search entries
const searchInput = document.getElementById('search-input');
if (searchInput) {
    // Set search value from URL
    const currentSearch = urlParams.get('search') || '';
    if (currentSearch) {
        searchInput.value = currentSearch;
    }

    searchInput.addEventListener('input', function(e) {
        const url = new URL(window.location.href);
        if (e.target.value) {
            url.searchParams.set('search', e.target.value);
        } else {
            url.searchParams.delete('search');
        }
        window.location.href = url.toString();
    });
}

// Open entry modal when clicking on entry card
document.querySelectorAll('.entry-card').forEach(card => {
    card.addEventListener('click', function() {
        currentTitleId = this.dataset.titleId;
        currentEntryId = this.dataset.entryId;
        currentProgress = parseFloat(this.dataset.progress) || 0;
        currentPages = parseInt(this.dataset.pages) || 0;

        const entryName = this.dataset.entryName;
        const entryPath = this.dataset.path;

        // Update modal content
        document.getElementById('modal-entry-name').textContent = entryName;
        document.getElementById('modal-entry-path').textContent = entryPath;
        document.getElementById('modal-entry-pages').textContent = `${currentPages} pages`;
        document.getElementById('continue-percent').textContent = `${currentProgress.toFixed(1)}%`;

        // Update read links
        document.getElementById('read-from-beginning').href = `/reader/${currentTitleId}/${currentEntryId}/1`;

        // Use the saved page directly from backend to avoid rounding errors
        const savedPage = parseInt(this.dataset.savedPage) || 0;
        const resumePage = savedPage > 0 ? savedPage : 1;
        document.getElementById('read-continue').href = `/reader/${currentTitleId}/${currentEntryId}/${resumePage}`;

        // Show or hide continue button based on progress
        if (currentProgress > 0 && currentProgress < 100) {
            document.getElementById('read-continue').style.display = 'inline-block';
        } else {
            document.getElementById('read-continue').style.display = 'none';
        }

        // Open modal
        UIkit.modal('#entry-modal').show();
    });
});

// Mark as read (100%)
async function markAsRead() {
    try {
        await fetch(`/api/progress/${currentTitleId}/${currentEntryId}`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                page: currentPages
            })
        });

        // Reload page to update progress badge
        window.location.reload();
    } catch (error) {
        console.error('Failed to mark as read:', error);
        alert('Failed to update progress');
    }
}

// Mark as unread (0%)
async function markAsUnread() {
    try {
        await fetch(`/api/progress/${currentTitleId}/${currentEntryId}`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                page: 0  // page 0 means delete progress
            })
        });

        // Reload page to update progress badge
        window.location.reload();
    } catch (error) {
        console.error('Failed to mark as unread:', error);
        alert('Failed to update progress');
    }
}
