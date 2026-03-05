(() => {
  if (window.__WS_DEBUG_HELPER_INSTALLED__) return;
  window.__WS_DEBUG_HELPER_INSTALLED__ = true;

  const OriginalWebSocket = window.WebSocket;
  let openCount = 0;
  let closeCount = 0;

  function ensureBadge() {
    let badge = document.getElementById('__ws_debug_badge__');
    if (!badge) {
      badge = document.createElement('div');
      badge.id = '__ws_debug_badge__';
      badge.style.position = 'fixed';
      badge.style.right = '10px';
      badge.style.bottom = '10px';
      badge.style.zIndex = '2147483647';
      badge.style.padding = '8px 10px';
      badge.style.borderRadius = '8px';
      badge.style.fontFamily = 'monospace';
      badge.style.fontSize = '12px';
      badge.style.background = 'rgba(0,0,0,0.78)';
      badge.style.color = '#9ef59e';
      badge.style.boxShadow = '0 2px 8px rgba(0,0,0,0.3)';
      badge.textContent = 'WS O:0 C:0';
      document.addEventListener('DOMContentLoaded', () => document.body.appendChild(badge), { once: true });
      if (document.body) document.body.appendChild(badge);
    }
    return badge;
  }

  function updateBadge() {
    const badge = ensureBadge();
    badge.textContent = `WS O:${openCount} C:${closeCount}`;
  }

  window.WebSocket = function(url, protocols) {
    const ws = protocols ? new OriginalWebSocket(url, protocols) : new OriginalWebSocket(url);
    console.log('[WS-DEBUG] create', url);

    ws.addEventListener('open', () => {
      openCount += 1;
      updateBadge();
      console.log('[WS-DEBUG] open', url);
    });

    ws.addEventListener('close', (ev) => {
      closeCount += 1;
      updateBadge();
      console.log('[WS-DEBUG] close', url, 'code=', ev.code, 'reason=', ev.reason);
    });

    ws.addEventListener('error', () => {
      console.log('[WS-DEBUG] error', url);
    });

    return ws;
  };
  window.WebSocket.prototype = OriginalWebSocket.prototype;
  Object.setPrototypeOf(window.WebSocket, OriginalWebSocket);
})();
