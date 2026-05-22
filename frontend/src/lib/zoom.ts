// frontend/src/lib/zoom.ts
//
// Block zoom (LNK-07). URL fragment shape: `#/pages/Foo#block=N` where N
// is the integer block id. svelte-spa-router consumes the leading `#/...`
// route fragment; whatever follows the SECOND `#` is the foliom-specific
// sub-fragment. After the route resolves and the page mounts, we add the
// `.zoomed` class to the targeted block and scroll it into view.
//
// Phase 2 ships scroll-and-highlight only. Subtree-only rendering (i.e.
// hiding everything else and rendering just the zoomed block's subtree)
// is deferred per 02-RESEARCH Open Question 2.

const ZOOMED_CLASS = 'zoomed';

let lastZoomedId: string | null = null;

function parseBlockFragment(hash: string): string | null {
  // hash looks like "#/pages/Foo#block=42" (or just "#/pages/Foo")
  const parts = hash.split('#').filter(Boolean);
  // parts[0] is the route ("/pages/Foo"), parts[1+] are sub-fragments.
  for (let i = 1; i < parts.length; i++) {
    const eq = parts[i].indexOf('=');
    if (eq < 0) continue;
    const key = parts[i].slice(0, eq);
    const val = parts[i].slice(eq + 1);
    if (key === 'block' && /^\d+$/.test(val)) {
      return val;
    }
  }
  return null;
}

export function applyZoomFromHash(): void {
  if (typeof document === 'undefined' || typeof window === 'undefined') return;

  // Clear previous zoom (route may have changed).
  if (lastZoomedId) {
    document.getElementById('block-' + lastZoomedId)?.classList.remove(ZOOMED_CLASS);
    lastZoomedId = null;
  }

  const id = parseBlockFragment(window.location.hash);
  if (!id) return;

  // The block may not yet be in the DOM if the page fetch hasn't resolved.
  // Use a microtask + rAF to give the renderer a chance, then a small
  // retry loop bounded at ~500ms total.
  let attempts = 0;
  const tryApply = () => {
    const el = document.getElementById('block-' + id);
    if (el) {
      el.classList.add(ZOOMED_CLASS);
      lastZoomedId = id;
      el.scrollIntoView({ block: 'center', behavior: 'instant' as ScrollBehavior });
      return;
    }
    if (attempts++ < 10) {
      window.setTimeout(tryApply, 50);
    }
  };
  window.requestAnimationFrame(tryApply);
}

export function installZoomListener(): void {
  if (typeof window === 'undefined') return;
  window.addEventListener('hashchange', applyZoomFromHash);
}

// Exposed for tests.
export const __test__ = { parseBlockFragment };
