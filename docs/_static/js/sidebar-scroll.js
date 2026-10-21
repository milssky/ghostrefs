// Preserve the reader's place in the navigation between documentation pages.
(() => {
  const key = 'ghostrefs:sidebar-scroll';

  document.addEventListener('DOMContentLoaded', () => {
    const sidebar = document.querySelector('#lside .sy-scrollbar');
    if (!sidebar) return;

    try {
      const saved = sessionStorage.getItem(key);
      if (saved !== null) {
        const position = Number(saved);
        if (Number.isFinite(position) && position >= 0) {
          sidebar.scrollTop = position;
        }
      }
    } catch {
      // Storage can be disabled in the reader's browser.
    }

    window.addEventListener('pagehide', () => {
      try {
        sessionStorage.setItem(key, String(sidebar.scrollTop));
      } catch {
        // Navigation remains usable without storage.
      }
    });
  });
})();
