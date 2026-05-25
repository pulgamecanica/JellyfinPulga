(function () {
  'use strict';

  const API = ApiClient.serverAddress();
  const headers = () => ({
    'Content-Type': 'application/json',
    'Authorization': `MediaBrowser Token="${ApiClient.accessToken()}"`
  });

  let currentUser = null;
  let currentRoom = null;
  let currentPmUser = null;

  document.querySelector('#PulgaPage').addEventListener('pageshow', async function () {
    const user = await ApiClient.getCurrentUser();
    currentUser = { id: user.Id, name: user.Name };
    setupTabs();
    setupChat();
    setupReports();
    setupMessages();
    loadMovies();
  });

  function setupTabs() {
    document.querySelectorAll('.pulga-tab').forEach(tab => {
      tab.addEventListener('click', () => {
        document.querySelectorAll('.pulga-tab').forEach(t => t.classList.remove('active'));
        tab.classList.add('active');
        document.querySelectorAll('.pulga-panel').forEach(p => p.classList.add('pulga-hidden'));
        document.getElementById('pulga-' + tab.dataset.tab).classList.remove('pulga-hidden');

        if (tab.dataset.tab === 'reports') loadReports();
        if (tab.dataset.tab === 'messages') loadConversations();
      });
    });
  }

  async function apiFetch(path, opts = {}) {
    const url = API + path;
    const res = await fetch(url, {
      method: opts.method || 'GET',
      headers: headers(),
      body: opts.body ? JSON.stringify(opts.body) : undefined
    });
    if (!res.ok) throw new Error('API error: ' + res.status);
    const ct = res.headers.get('content-type') || '';
    return ct.includes('json') ? res.json() : res.text();
  }

  // --- Chat ---

  async function loadMovies() {
    const items = await apiFetch('/Items?Recursive=true&IncludeItemTypes=Movie&Fields=Path&SortBy=SortName&SortOrder=Ascending&Limit=500');
    window._pulgaMovies = items.Items;
    renderMovieList(items.Items);
  }

  function renderMovieList(movies) {
    const list = document.getElementById('pulga-room-list');
    list.innerHTML = movies.map(m => `
      <div class="pulga-list-item" data-room="${m.Id}" data-name="${esc(m.Name)}">
        <div>
          <h4>${esc(m.Name)}</h4>
        </div>
        <div class="pulga-actions">
          <button class="pulga-btn-sm pulga-report-btn" data-id="${m.Id}" data-name="${esc(m.Name)}">Report</button>
        </div>
      </div>`).join('');

    list.querySelectorAll('.pulga-list-item').forEach(el => {
      el.addEventListener('click', (e) => {
        if (e.target.classList.contains('pulga-report-btn')) return;
        openChatRoom(el.dataset.room, el.dataset.name);
      });
    });

    list.querySelectorAll('.pulga-report-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        openReportModal(btn.dataset.id, btn.dataset.name);
      });
    });
  }

  function setupChat() {
    const searchInput = document.getElementById('pulga-movie-search');
    searchInput.addEventListener('input', () => {
      const q = searchInput.value.toLowerCase();
      const filtered = (window._pulgaMovies || []).filter(m => m.Name.toLowerCase().includes(q));
      renderMovieList(filtered);
    });

    document.getElementById('pulga-chat-back').addEventListener('click', closeChatRoom);

    const input = document.getElementById('pulga-chat-input');
    const send = document.getElementById('pulga-chat-send');
    const doSend = async () => {
      const text = input.value.trim();
      if (!text || !currentRoom) return;
      await apiFetch('/Pulga/Chat/' + currentRoom + '/Send', {
        method: 'POST',
        body: { username: currentUser.name, content: text }
      });
      input.value = '';
      loadChatMessages(currentRoom);
    };
    send.addEventListener('click', doSend);
    input.addEventListener('keydown', e => { if (e.key === 'Enter') doSend(); });
  }

  function openChatRoom(roomId, title) {
    currentRoom = roomId;
    document.getElementById('pulga-chat-title').textContent = title;
    document.getElementById('pulga-room-list').classList.add('pulga-hidden');
    document.querySelector('.pulga-search').classList.add('pulga-hidden');
    document.getElementById('pulga-chat-area').classList.remove('pulga-hidden');
    loadChatMessages(roomId);
  }

  function closeChatRoom() {
    currentRoom = null;
    document.getElementById('pulga-chat-area').classList.add('pulga-hidden');
    document.getElementById('pulga-room-list').classList.remove('pulga-hidden');
    document.querySelector('.pulga-search').classList.remove('pulga-hidden');
  }

  async function loadChatMessages(roomId) {
    const msgs = await apiFetch('/Pulga/Chat/' + roomId + '/Messages?limit=50');
    const container = document.getElementById('pulga-chat-messages');
    container.innerHTML = msgs.map(m => `
      <div class="pulga-msg">
        <span class="sender">${esc(m.Username)}</span>
        <span class="time">${new Date(m.CreatedAt).toLocaleTimeString()}</span>
        <div class="text">${esc(m.Content)}</div>
      </div>`).join('');
    container.scrollTop = container.scrollHeight;
  }

  // --- Reports ---

  function setupReports() {
    document.getElementById('pulga-report-filter').addEventListener('change', loadReports);

    document.getElementById('pulga-report-cancel').addEventListener('click', () => {
      document.getElementById('pulga-report-modal').classList.add('pulga-hidden');
    });

    document.getElementById('pulga-report-submit').addEventListener('click', async () => {
      const itemId = document.getElementById('pulga-report-item-id').value;
      const itemName = document.getElementById('pulga-report-item-name').value;
      await apiFetch('/Pulga/Reports', {
        method: 'POST',
        body: {
          itemId,
          itemName,
          reporterName: currentUser.name,
          reason: document.getElementById('pulga-report-reason').value,
          details: document.getElementById('pulga-report-details').value
        }
      });
      document.getElementById('pulga-report-modal').classList.add('pulga-hidden');
      document.getElementById('pulga-report-details').value = '';
      Dashboard.alert('Report submitted');
    });
  }

  function openReportModal(itemId, itemName) {
    document.getElementById('pulga-report-item-id').value = itemId;
    document.getElementById('pulga-report-item-name').value = itemName;
    document.getElementById('pulga-report-modal').classList.remove('pulga-hidden');
  }

  async function loadReports() {
    const filter = document.getElementById('pulga-report-filter').value;
    const qs = filter ? '?status=' + filter : '';
    const reports = await apiFetch('/Pulga/Reports' + qs);
    const list = document.getElementById('pulga-report-list');

    if (reports.length === 0) {
      list.innerHTML = '<p style="opacity:0.5">No reports.</p>';
      return;
    }

    list.innerHTML = reports.map(r => `
      <div class="pulga-list-item">
        <div>
          <h4>${esc(r.ItemName)}</h4>
          <div class="meta">
            <span class="pulga-badge pulga-badge-${r.Status}">${r.Status}</span>
            ${esc(r.Reason)} — by ${esc(r.ReporterName)}
            ${r.Details ? '<br>' + esc(r.Details) : ''}
          </div>
        </div>
      </div>`).join('');
  }

  // --- Messages ---

  function setupMessages() {
    const input = document.getElementById('pulga-pm-input');
    const send = document.getElementById('pulga-pm-send');
    const doSend = async () => {
      const text = input.value.trim();
      if (!text || !currentPmUser) return;
      const toUser = currentPmUser;
      await apiFetch('/Pulga/Messages/' + toUser.id + '/Send', {
        method: 'POST',
        body: { fromUsername: currentUser.name, toUsername: toUser.name, content: text }
      });
      input.value = '';
      loadPmMessages(toUser);
    };
    send.addEventListener('click', doSend);
    input.addEventListener('keydown', e => { if (e.key === 'Enter') doSend(); });
  }

  async function loadConversations() {
    const users = await apiFetch('/Users');
    const convos = await apiFetch('/Pulga/Messages/Conversations');

    const convoMap = {};
    convos.forEach(c => { convoMap[c.UserId] = c; });

    const list = document.getElementById('pulga-convo-list');
    list.innerHTML = users
      .filter(u => u.Id !== currentUser.id)
      .map(u => {
        const c = convoMap[u.Id];
        const unread = c ? c.Unread : 0;
        return `
          <div class="pulga-list-item" data-uid="${u.Id}" data-uname="${esc(u.Name)}">
            <div><h4>${esc(u.Name)}</h4></div>
            ${unread > 0 ? '<span class="pulga-unread">' + unread + '</span>' : ''}
          </div>`;
      }).join('');

    list.querySelectorAll('.pulga-list-item').forEach(el => {
      el.addEventListener('click', () => {
        openPm({ id: el.dataset.uid, name: el.dataset.uname });
      });
    });
  }

  function openPm(user) {
    currentPmUser = user;
    document.getElementById('pulga-pm-header').textContent = user.name;
    document.getElementById('pulga-pm-area').classList.remove('pulga-hidden');
    loadPmMessages(user);
  }

  async function loadPmMessages(user) {
    const msgs = await apiFetch('/Pulga/Messages/' + user.id + '?limit=50');
    const container = document.getElementById('pulga-pm-messages');
    container.innerHTML = msgs.map(m => `
      <div class="pulga-msg">
        <span class="sender">${esc(m.FromUsername)}</span>
        <span class="time">${new Date(m.CreatedAt).toLocaleTimeString()}</span>
        <div class="text">${esc(m.Content)}</div>
      </div>`).join('');
    container.scrollTop = container.scrollHeight;
    await apiFetch('/Pulga/Messages/' + user.id + '/Read', { method: 'POST' });
  }

  function esc(str) {
    const d = document.createElement('div');
    d.textContent = str;
    return d.innerHTML;
  }
})();
