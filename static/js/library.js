// Alpine.js component for library page
function libraryData() {
    return {
        searchQuery: '',

        // Check if a title matches the search query
        matchesSearch(titleName) {
            if (!this.searchQuery.trim()) {
                return true;
            }
            const regex = new RegExp(this.searchQuery, 'i');
            return regex.test(titleName);
        },

        // Handle sort dropdown change
        handleSortChange(event) {
            const [sort, ascend] = event.target.value.split(':');
            const url = new URL(window.location.href);
            url.searchParams.set('sort', sort);
            url.searchParams.set('ascend', ascend);
            window.location.href = url.toString();
        }
    };
}
