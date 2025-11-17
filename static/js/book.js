// Alpine.js component for book page
function bookData() {
    return {
        searchQuery: '',
        modalData: {
            titleId: '',
            entryId: '',
            entryName: '',
            entryPath: '',
            pages: 0,
            progress: 0,
            savedPage: 0,
            resumePage: 1
        },

        // Check if an entry matches the search query
        matchesSearch(entryName) {
            if (!this.searchQuery.trim()) {
                return true;
            }
            const regex = new RegExp(this.searchQuery, 'i');
            return regex.test(entryName);
        },

        // Handle sort dropdown change
        handleSortChange(event) {
            const [sort, ascend] = event.target.value.split(':');
            const url = new URL(window.location.href);
            url.searchParams.set('sort', sort);
            url.searchParams.set('ascend', ascend);
            window.location.href = url.toString();
        },

        // Open entry modal
        openModal(titleId, entryId, entryName, entryPath, pages, progress, savedPage) {
            this.modalData = {
                titleId,
                entryId,
                entryName,
                entryPath,
                pages,
                progress,
                savedPage,
                resumePage: savedPage > 0 ? savedPage : 1
            };

            // Open modal using UIkit
            UIkit.modal('#entry-modal').show();
        },

        // Mark entry as read (100%)
        async markAsRead() {
            try {
                await fetch(`/api/progress/${this.modalData.titleId}/${this.modalData.entryId}`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({
                        page: this.modalData.pages
                    })
                });

                // Reload page to update progress badge
                window.location.reload();
            } catch (error) {
                console.error('Failed to mark as read:', error);
                alert('Failed to update progress');
            }
        },

        // Mark entry as unread (0%)
        async markAsUnread() {
            try {
                await fetch(`/api/progress/${this.modalData.titleId}/${this.modalData.entryId}`, {
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
    };
}
