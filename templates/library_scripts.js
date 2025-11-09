function changeSort(sortMethod) {
    const url = new URL(window.location.href);
    url.searchParams.set('sort', sortMethod);
    window.location.href = url.toString();
}

// Set selected option based on URL parameter
const urlParams = new URLSearchParams(window.location.search);
const currentSort = urlParams.get('sort') || 'name';
document.getElementById('sort-select').value = currentSort;
