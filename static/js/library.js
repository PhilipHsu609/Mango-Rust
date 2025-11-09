// Search functionality
document.addEventListener('DOMContentLoaded', function() {
    const searchInput = document.querySelector('.uk-search-input');
    const items = document.querySelectorAll('.item');
    const titles = [];

    // Collect all title texts
    document.querySelectorAll('.uk-card-title').forEach(function(el) {
        titles.push(el.textContent);
    });

    if (searchInput) {
        searchInput.addEventListener('input', function(e) {
            const input = e.target.value.trim();
            const regex = new RegExp(input, 'i');

            if (input === '') {
                // Show all items
                items.forEach(function(item) {
                    item.removeAttribute('hidden');
                });
            } else {
                // Filter items
                items.forEach(function(item, i) {
                    if (titles[i] && titles[i].match(regex)) {
                        item.removeAttribute('hidden');
                    } else {
                        item.setAttribute('hidden', '');
                    }
                });
            }
        });
    }
});

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
    const urlParams = new URLSearchParams(window.location.search);
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
