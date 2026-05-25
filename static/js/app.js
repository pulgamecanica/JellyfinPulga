const API = '/api';
let currentUser = { id: '', name: '' };
let currentChatRoom = null;
let currentPmUser = null;

document.addEventListener('DOMContentLoaded', async () => {
  await loadUsers();
  setupNavigation();
  setupChat();
  setupReportModal();
  setupMessages();
  loadMovies();
});

async function api(path, opts = {}) {
  const url = new URL(API + path, window.location.origin);
  if (opts.params) {
    Object.entries(opts.params).forEach(([k, v]) => url.searchParams.set(k, v));
  }
  const res = await fetch(url, {
    method: opts.method || 'GET',
    headers: opts.body ? { 'Content-Type': 'application/json' } : {},
    body: opts.body ? JSON.stringify(opts.body) : undefined,
  });
  if (!res.ok) throw new Error(`API error: ${res.status}`);
  const ct = res.headers.get('content-type') || '';
  return ct.includes('json') ? res.json() : res.text();
}

async function loadUsers() {
  const users = await api('/users');
  const select = document.getElementById('current-user');
  select.innerHTML = users.map(u =>
    `<option value="${u.Id}" data-name="${u.Name}">${u.Name}</option>`
  ).join('');
  select.addEventListener('change', () => {
    const opt = select.selectedOptions[0];
    currentUser = { id: opt.value, name: opt.dataset.name };
  });
  if (users.length > 0) {
    currentUser = { id: users[0].Id, name: users[0].Name };
  }
}

function setupNavigation() {
  document.querySelectorAll('#sidebar a[data-view]').forEach(link => {
    link.addEventListener('click', e => {
      e.preventDefault();
      const view = link.dataset.view;
      document.querySelectorAll('#sidebar a').forEach(a => a.classList.remove('active'));
      link.classList.add('active');
      document.querySelectorAll('.view').forEach(v => v.classList.add('hidden'));
      document.getElementById(`view-${view}`).classList.remove('hidden');

      if (view === 'movies') loadMovies();
      else if (view === 'flagged') loadFlagged();
      else if (view === 'reports') loadReports();
      else if (view === 'users') loadUserList();
      else if (view === 'messages') loadConversations();
    });
  });
}

async function loadMovies() {
  const movies = await api('/movies');
  const container = document.getElementById('movie-list');
  renderMovieGrid(container, movies);

  document.getElementById('movie-search').addEventListener('input', e => {
    const q = e.target.value.toLowerCase();
    const filtered = movies.filter(m => m.Name.toLowerCase().includes(q));
    renderMovieGrid(container, filtered);
  });
}

function renderMovieGrid(container, movies) {
  container.innerHTML = movies.map(m => {
    const tags = (m.Tags || []).map(t => {
      const cls = t === 'needs-review' ? 'tag needs-review' : 'tag';
      return `<span class="${cls}">${esc(t)}</span>`;
    }).join('');
    const path = m.Path || '';
    return `
      <div class="item-card">
        <h3>${esc(m.Name)}</h3>
        <div class="path">${esc(path)}</div>
        <div class="tags">${tags}</div>
        <div class="actions">
          <button onclick="openChat('${m.Id}', '${esc(m.Name)}')">Chat</button>
          <button onclick="openReport('${m.Id}', '${esc(m.Name)}')">Report</button>
        </div>
      </div>`;
  }).join('');
}

async function loadFlagged() {
  const items = await api('/movies/flagged');
  const container = document.getElementById('flagged-list');
  if (items.length === 0) {
    container.innerHTML = '<p style="color:var(--text-dim)">No flagged items.</p>';
    return;
  }
  container.innerHTML = items.map(item => `
    <div class="list-item">
      <div class="info">
        <h4>${esc(item.Name)}</h4>
        <div class="meta">${esc(item.Path || '')} &mdash; ${esc(item.Type)}</div>
      </div>
      <div class="actions">
        <button onclick="openChat('${item.Id}', '${esc(item.Name)}')">Chat</button>
        <button onclick="openReport('${item.Id}', '${esc(item.Name)}')">Report</button>
      </div>
    </div>`).join('');

  document.getElementById('export-flagged').onclick = async () => {
    const tsv = await api('/export/flagged');
    const blob = new Blob([tsv], { type: 'text/tab-separated-values' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'flagged_items.tsv';
    a.click();
  };
}

async function loadReports() {
  const filter = document.getElementById('report-filter').value;
  const params = filter ? { status: filter } : {};
  const reports = await api('/reports', { params });
  const container = document.getElementById('report-list');

  if (reports.length === 0) {
    container.innerHTML = '<p style="color:var(--text-dim)">No reports.</p>';
    return;
  }

  container.innerHTML = reports.map(r => `
    <div class="list-item">
      <div class="info">
        <h4>${esc(r.item_name)}</h4>
        <div class="meta">
          <span class="status-badge status-${r.status}">${r.status}</span>
          ${esc(r.reason)} &mdash; reported by ${esc(r.reporter_name)}
          ${r.details ? '<br>' + esc(r.details) : ''}
        </div>
      </div>
      <div class="actions">
        <select onchange="updateReportStatus(${r.id}, this.value)">
          <option value="">Change status...</option>
          <option value="open">Open</option>
          <option value="reviewed">Reviewed</option>
          <option value="resolved">Resolved</option>
          <option value="dismissed">Dismissed</option>
        </select>
      </div>
    </div>`).join('');

  document.getElementById('report-filter').onchange = () => loadReports();
}

async function updateReportStatus(id, status) {
  if (!status) return;
  await api(`/reports/${id}/status`, { method: 'POST', body: { status } });
  loadReports();
}

async function loadUserList() {
  const users = await api('/users');
  const container = document.getElementById('user-list');
  container.innerHTML = users.map(u => `
    <div class="list-item">
      <div class="info">
        <h4>${esc(u.Name)}</h4>
        <div class="meta">ID: ${u.Id}</div>
      </div>
      <div class="actions">
        <button onclick="startPm('${u.Id}', '${esc(u.Name)}')">Message</button>
        <button class="danger" onclick="blockUser('${u.Id}')">Block</button>
      </div>
    </div>`).join('');
}

function setupChat() {
  document.getElementById('chat-close').onclick = () => {
    document.getElementById('chat-panel').classList.add('hidden');
    currentChatRoom = null;
  };

  const input = document.getElementById('chat-input');
  const send = document.getElementById('chat-send');

  const doSend = async () => {
    const text = input.value.trim();
    if (!text || !currentChatRoom) return;
    await api(`/chat/${currentChatRoom}/send`, {
      method: 'POST',
      body: { user_id: currentUser.id, username: currentUser.name, content: text },
    });
    input.value = '';
    loadChatMessages(currentChatRoom);
  };

  send.onclick = doSend;
  input.addEventListener('keydown', e => { if (e.key === 'Enter') doSend(); });
}

function openChat(roomId, title) {
  currentChatRoom = roomId;
  document.getElementById('chat-title').textContent = title;
  document.getElementById('chat-panel').classList.remove('hidden');
  loadChatMessages(roomId);
}

async function loadChatMessages(roomId) {
  const msgs = await api(`/chat/${roomId}/messages`, {
    params: { user_id: currentUser.id, username: currentUser.name, limit: '50' },
  });
  const container = document.getElementById('chat-messages');
  container.innerHTML = msgs.map(m => `
    <div class="msg">
      <span class="sender">${esc(m.username)}</span>
      <span class="time">${new Date(m.created_at).toLocaleTimeString()}</span>
      <div class="text">${esc(m.content)}</div>
    </div>`).join('');
  container.scrollTop = container.scrollHeight;
}

function setupReportModal() {
  document.getElementById('report-cancel').onclick = () => {
    document.getElementById('report-modal').classList.add('hidden');
  };
  document.getElementById('report-submit').onclick = async () => {
    const itemId = document.getElementById('report-item-id').value;
    const itemName = document.getElementById('report-item-name').value;
    const reason = document.getElementById('report-reason').value;
    const details = document.getElementById('report-details').value;

    await api('/reports', {
      method: 'POST',
      body: {
        item_id: itemId,
        item_name: itemName,
        reporter_id: currentUser.id,
        reporter_name: currentUser.name,
        reason,
        details,
      },
    });
    document.getElementById('report-modal').classList.add('hidden');
    document.getElementById('report-details').value = '';
  };
}

function openReport(itemId, itemName) {
  document.getElementById('report-item-id').value = itemId;
  document.getElementById('report-item-name').value = itemName;
  document.getElementById('report-modal').classList.remove('hidden');
}

function setupMessages() {
  const input = document.getElementById('pm-input');
  const send = document.getElementById('pm-send');

  const doSend = async () => {
    const text = input.value.trim();
    if (!text || !currentPmUser) return;
    await api(`/pm/${currentPmUser}/send`, {
      method: 'POST',
      body: { user_id: currentUser.id, username: currentUser.name, content: text },
    });
    input.value = '';
    loadPmMessages(currentPmUser);
  };

  send.onclick = doSend;
  input.addEventListener('keydown', e => { if (e.key === 'Enter') doSend(); });
}

async function loadConversations() {
  const convos = await api('/pm/conversations', {
    params: { user_id: currentUser.id, username: currentUser.name },
  });
  const container = document.getElementById('conversation-list');

  const users = await api('/users');
  const userItems = users
    .filter(u => u.Id !== currentUser.id)
    .map(u => {
      const convo = convos.find(c => c.user_id === u.Id);
      const unread = convo ? convo.unread : 0;
      return { id: u.Id, name: u.Name, unread };
    });

  container.innerHTML = userItems.map(u => `
    <div class="convo-item" onclick="startPm('${u.id}', '${esc(u.name)}')">
      <span class="name">${esc(u.name)}</span>
      ${u.unread > 0 ? `<span class="unread">${u.unread}</span>` : ''}
    </div>`).join('');
}

function startPm(userId, userName) {
  document.querySelectorAll('#sidebar a').forEach(a => a.classList.remove('active'));
  document.querySelector('#sidebar a[data-view="messages"]').classList.add('active');
  document.querySelectorAll('.view').forEach(v => v.classList.add('hidden'));
  document.getElementById('view-messages').classList.remove('hidden');

  currentPmUser = userId;
  document.getElementById('message-header').textContent = userName;
  document.getElementById('message-input-area').classList.remove('hidden');
  loadPmMessages(userId);
  loadConversations();
}

async function loadPmMessages(userId) {
  const msgs = await api(`/pm/${userId}/messages`, {
    params: { user_id: currentUser.id, username: currentUser.name, limit: '50' },
  });
  const container = document.getElementById('message-content');
  container.innerHTML = msgs.map(m => `
    <div class="msg">
      <span class="sender">${esc(m.from_username)}</span>
      <span class="time">${new Date(m.created_at).toLocaleTimeString()}</span>
      <div class="text">${esc(m.content)}</div>
    </div>`).join('');
  container.scrollTop = container.scrollHeight;

  await api(`/pm/${userId}/read`, {
    method: 'POST',
    params: { user_id: currentUser.id, username: currentUser.name },
  });
}

async function blockUser(userId) {
  await api(`/block/${userId}`, {
    method: 'POST',
    params: { user_id: currentUser.id, username: currentUser.name },
  });
  loadUserList();
}

function esc(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}
