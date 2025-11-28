// Cache Debug Page Alpine.js component

function cacheDebugData() {
    return {
        loading: false,
        message: '',
        messageType: 'success',

        async refreshStats() {
            this.loading = true;
            this.message = '';
            try {
                // Reload the page to get fresh stats
                window.location.reload();
            } catch (error) {
                this.showError('Failed to refresh: ' + error.message);
            } finally {
                this.loading = false;
            }
        },

        async saveLibraryCache() {
            this.loading = true;
            this.message = '';
            try {
                const response = await fetch('/api/cache/save-library', {
                    method: 'POST'
                });

                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}`);
                }

                this.showSuccess('Library cache saved successfully');
                // Refresh stats after a brief delay
                setTimeout(() => window.location.reload(), 1000);
            } catch (error) {
                this.showError('Failed to save library cache: ' + error.message);
            } finally {
                this.loading = false;
            }
        },

        async loadLibraryCache() {
            this.loading = true;
            this.message = '';
            try {
                const response = await fetch('/api/cache/load-library', {
                    method: 'POST'
                });

                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}`);
                }

                const result = await response.json();
                if (result.loaded) {
                    this.showSuccess('Library loaded from cache successfully');
                } else {
                    this.showError('Cache miss or invalid cache');
                }

                // Refresh stats after a brief delay
                setTimeout(() => window.location.reload(), 1000);
            } catch (error) {
                this.showError('Failed to load library cache: ' + error.message);
            } finally {
                this.loading = false;
            }
        },

        async clearCache() {
            // This is called from the modal toggle, actual clear happens in confirmClearCache
        },

        async confirmClearCache() {
            this.loading = true;
            this.message = '';
            try {
                const response = await fetch('/api/cache/clear', {
                    method: 'POST'
                });

                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}`);
                }

                this.showSuccess('Cache cleared successfully');
                // Refresh stats after a brief delay
                setTimeout(() => window.location.reload(), 1000);
            } catch (error) {
                this.showError('Failed to clear cache: ' + error.message);
            } finally {
                this.loading = false;
            }
        },

        showSuccess(msg) {
            this.message = msg;
            this.messageType = 'success';
        },

        showError(msg) {
            this.message = msg;
            this.messageType = 'error';
        }
    };
}
