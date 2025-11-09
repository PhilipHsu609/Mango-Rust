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
