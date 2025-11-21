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

// Alpine.js component for tags management
function tagsData() {
    return {
        tags: [],
        allTags: [], // All existing tags in the system for suggestions
        showAddForm: false,
        newTag: '',
        addingTag: false,
        deletingTag: null,
        showSuggestions: false,

        async init() {
            // Get title ID from page (injected via template)
            const titleId = this.$root.dataset.titleId;
            if (titleId) {
                await this.loadTags(titleId);
            }
            // Load all tags for autocomplete
            await this.loadAllTags();
        },

        // Get filtered suggestions based on input
        get filteredSuggestions() {
            if (!this.newTag.trim()) {
                return this.allTags.filter(tag => !this.tags.includes(tag));
            }
            const searchLower = this.newTag.toLowerCase();
            return this.allTags.filter(tag =>
                tag.toLowerCase().includes(searchLower) && !this.tags.includes(tag)
            );
        },

        async loadTags(titleId) {
            try {
                const response = await fetch(`/api/tags/${titleId}`);
                if (response.ok) {
                    this.tags = await response.json();
                }
            } catch (error) {
                console.error('Failed to load tags:', error);
            }
        },

        async loadAllTags() {
            try {
                const response = await fetch('/api/tags');
                if (response.ok) {
                    const data = await response.json();
                    // Extract just the tag names from the array of {tag, count} objects
                    this.allTags = data.map(item => item.tag);
                }
            } catch (error) {
                console.error('Failed to load all tags:', error);
            }
        },

        selectSuggestion(tag) {
            this.newTag = tag;
            this.showSuggestions = false;
            // Focus remains on input for user to press Enter or click Add
        },

        async addTag() {
            const tag = this.newTag.trim();
            if (!tag) return;

            const titleId = this.$root.dataset.titleId;
            this.addingTag = true;

            try {
                const response = await fetch(`/api/admin/tags/${titleId}/${encodeURIComponent(tag)}`, {
                    method: 'PUT'
                });

                if (response.ok) {
                    // Add tag to local array
                    this.tags.push(tag);
                    this.tags.sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }));

                    // Reset form
                    this.newTag = '';
                    this.showAddForm = false;
                } else {
                    const error = await response.text();
                    alert(`Failed to add tag: ${error}`);
                }
            } catch (error) {
                console.error('Failed to add tag:', error);
                alert('Failed to add tag');
            } finally {
                this.addingTag = false;
            }
        },

        async deleteTag(tag) {
            if (!confirm(`Remove tag "${tag}"?`)) return;

            const titleId = this.$root.dataset.titleId;
            this.deletingTag = tag;

            try {
                const response = await fetch(`/api/admin/tags/${titleId}/${encodeURIComponent(tag)}`, {
                    method: 'DELETE'
                });

                if (response.ok) {
                    // Remove tag from local array
                    const index = this.tags.indexOf(tag);
                    if (index > -1) {
                        this.tags.splice(index, 1);
                    }
                } else {
                    const error = await response.text();
                    alert(`Failed to delete tag: ${error}`);
                }
            } catch (error) {
                console.error('Failed to delete tag:', error);
                alert('Failed to delete tag');
            } finally {
                this.deletingTag = null;
            }
        },

        cancelAdd() {
            this.newTag = '';
            this.showAddForm = false;
        }
    };
}
