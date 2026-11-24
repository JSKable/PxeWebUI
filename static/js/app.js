// PxeWebUI Frontend Controller

let currentUser = null;
let allIsos = [];
let allPresets = [];
let currentCategory = 'all';
let statsPollInterval = null;
let downloadsPollInterval = null;
const knownCompletedDownloadIds = new Set();

// Initialize on page load
document.addEventListener('DOMContentLoaded', async () => {
    await checkAuth();
    if (currentUser) {
        initApp();
    }
});

// --- Authentication ---
async function checkAuth() {
    try {
        const token = localStorage.getItem('pxe_token');
        const headers = token ? { 'Authorization': `Bearer ${token}` } : {};
        const res = await fetch('/api/auth/me', { headers });
        if (res.ok) {
            currentUser = await res.json();
            updateUserUI();
            document.getElementById('loginModal').classList.add('hidden');
        } else {
            document.getElementById('loginModal').classList.remove('hidden');
        }
    } catch (e) {
        document.getElementById('loginModal').classList.remove('hidden');
    }
}

function updateUserUI() {
    if (!currentUser) return;
    document.getElementById('currentUsername').textContent = currentUser.username;
    document.getElementById('dropdownUsername').textContent = currentUser.username;
    document.getElementById('dropdownRole').textContent = currentUser.role.toUpperCase();
    document.getElementById('userAvatar').textContent = currentUser.username.charAt(0).toUpperCase();

    // Hide user management tab for non-admins
    const userTabBtn = document.getElementById('tab-btn-users');
    if (currentUser.role !== 'admin' && userTabBtn) {
        userTabBtn.classList.add('hidden');
    }
}

async function submitLogin(e) {
    e.preventDefault();
    const errBox = document.getElementById('loginError');
    errBox.classList.add('hidden');

    const username = document.getElementById('loginUsername').value.trim();
    const password = document.getElementById('loginPassword').value;

    try {
        const res = await fetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
        });

        if (res.ok) {
            const data = await res.json();
            localStorage.setItem('pxe_token', data.token);
            currentUser = data.user;
            updateUserUI();
            document.getElementById('loginModal').classList.add('hidden');
            initApp();
        } else {
            const err = await res.json();
            errBox.textContent = err.error || 'Invalid credentials';
            errBox.classList.remove('hidden');
        }
    } catch (err) {
        errBox.textContent = 'Server connection error';
        errBox.classList.remove('hidden');
    }
}

function fillDefaultCreds() {
    document.getElementById('loginUsername').value = 'Admin';
    document.getElementById('loginPassword').value = 'PxeWebUIadmin';
}

async function logout() {
    const token = localStorage.getItem('pxe_token');
    await fetch('/api/auth/logout', {
        method: 'POST',
        headers: token ? { 'Authorization': `Bearer ${token}` } : {}
    });
    localStorage.removeItem('pxe_token');
    currentUser = null;
    document.getElementById('loginModal').classList.remove('hidden');
    closeUserDropdown();
}

function toggleUserDropdown() {
    const dd = document.getElementById('userDropdown');
    dd.classList.toggle('hidden');
}

function closeUserDropdown() {
    document.getElementById('userDropdown').classList.add('hidden');
}

// Close dropdown on outside click
window.addEventListener('click', (e) => {
    const btn = document.getElementById('userMenuBtn');
    const dd = document.getElementById('userDropdown');
    if (btn && dd && !btn.contains(e.target) && !dd.contains(e.target)) {
        dd.classList.add('hidden');
    }
});

// API Fetch Helper
async function apiFetch(url, options = {}) {
    const token = localStorage.getItem('pxe_token');
    const headers = options.headers || {};
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }
    options.headers = headers;

    const res = await fetch(url, options);
    if (res.status === 401) {
        currentUser = null;
        document.getElementById('loginModal').classList.remove('hidden');
        throw new Error('Unauthorized');
    }
    return res;
}

// --- App Initialization ---
async function initApp() {
    await Promise.all([
        loadIsos(),
        loadPresets(),
        loadSettings(),
        loadStats(),
        loadDownloads()
    ]);

    if (!statsPollInterval) {
        statsPollInterval = setInterval(loadStats, 4000);
    }
    if (!downloadsPollInterval) {
        downloadsPollInterval = setInterval(loadDownloads, 1500);
    }
}

// --- Navigation Tabs ---
function switchTab(tabId) {
    closeUserDropdown();
    const tabs = ['isos', 'downloads', 'preview', 'logs', 'users', 'settings', 'guide'];
    tabs.forEach(t => {
        const tabEl = document.getElementById(`tab-${t}`);
        const btnEl = document.getElementById(`tab-btn-${t}`);
        if (tabEl) tabEl.classList.add('hidden');
        if (btnEl) {
            btnEl.classList.remove('active-tab', 'border-sky-500', 'text-sky-400');
            btnEl.classList.add('border-transparent', 'text-gray-400');
        }
    });

    const activeTab = document.getElementById(`tab-${tabId}`);
    const activeBtn = document.getElementById(`tab-btn-${tabId}`);
    if (activeTab) activeTab.classList.remove('hidden');
    if (activeBtn) {
        activeBtn.classList.add('active-tab', 'border-sky-500', 'text-sky-400');
        activeBtn.classList.remove('border-transparent', 'text-gray-400');
    }

    if (tabId === 'preview') loadPxePreview();
    if (tabId === 'logs') loadLogs();
    if (tabId === 'users') loadUsers();
}

// --- ISO Library ---
async function loadIsos() {
    try {
        const res = await apiFetch('/api/isos');
        if (res.ok) {
            allIsos = await res.json();
            document.getElementById('isoCountBadge').textContent = allIsos.length;
            renderIsos();
        }
    } catch (e) {
        console.error('Failed to load ISOs:', e);
    }
}

function renderIsos() {
    const grid = document.getElementById('isoCardsGrid');
    const empty = document.getElementById('isoEmptyState');
    grid.innerHTML = '';

    const query = (document.getElementById('searchInput').value || '').toLowerCase();
    const filtered = allIsos.filter(iso => {
        const matchesCategory = currentCategory === 'all' || iso.category === currentCategory;
        const matchesQuery = !query ||
            iso.title.toLowerCase().includes(query) ||
            iso.filename.toLowerCase().includes(query) ||
            iso.description.toLowerCase().includes(query);
        return matchesCategory && matchesQuery;
    });

    if (filtered.length === 0) {
        empty.classList.remove('hidden');
        return;
    }
    empty.classList.add('hidden');

    filtered.forEach(iso => {
        const card = document.createElement('div');
        card.className = `bg-cardbg rounded-2xl border ${iso.is_enabled ? 'border-gray-800' : 'border-gray-800/40 opacity-60'} p-5 flex flex-col justify-between hover:border-gray-700 transition duration-150 shadow-lg`;

        const categoryColors = {
            linux: 'bg-emerald-950 text-emerald-400 border-emerald-800/50',
            windows: 'bg-blue-950 text-blue-400 border-blue-800/50',
            hypervisor: 'bg-purple-950 text-purple-400 border-purple-800/50',
            utility: 'bg-amber-950 text-amber-400 border-amber-800/50',
            custom: 'bg-slate-800 text-slate-300 border-slate-700',
        };
        const catBadgeClass = categoryColors[iso.category] || categoryColors.custom;

        const sizeFormatted = formatBytes(iso.file_size);

        card.innerHTML = `
            <div>
                <!-- Top Row: Category, Default Star, Toggle Switch -->
                <div class="flex items-center justify-between mb-3">
                    <div class="flex items-center space-x-2">
                        <span class="px-2 py-0.5 text-[11px] font-semibold uppercase tracking-wider rounded-md border ${catBadgeClass}">
                            ${iso.category}
                        </span>
                        ${iso.is_default ? `
                            <span class="px-2 py-0.5 text-[11px] font-bold rounded-md bg-amber-500/20 text-amber-300 border border-amber-500/40 flex items-center space-x-1">
                                <span>★</span><span>DEFAULT</span>
                            </span>
                        ` : ''}
                    </div>

                    <!-- Enable/Disable Switch -->
                    <label class="relative inline-flex items-center cursor-pointer">
                        <input type="checkbox" ${iso.is_enabled ? 'checked' : ''} onchange="toggleIsoEnabled('${iso.id}', this.checked)" class="sr-only peer">
                        <div class="w-9 h-5 bg-gray-800 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-sky-600"></div>
                    </label>
                </div>

                <!-- Title & Filename -->
                <h4 class="text-base font-bold text-white tracking-tight line-clamp-1">${escapeHtml(iso.title)}</h4>
                <p class="text-xs font-mono text-gray-400 mt-0.5 line-clamp-1">${escapeHtml(iso.filename)}</p>

                <!-- Description -->
                <p class="text-xs text-gray-300 mt-2 line-clamp-2 h-8 leading-relaxed">
                    ${iso.description ? escapeHtml(iso.description) : '<span class="text-gray-500 italic">No description provided</span>'}
                </p>

                <!-- Badges: Size, Boot Mode, Arch -->
                <div class="mt-4 pt-3 border-t border-gray-800/80 flex flex-wrap gap-1.5 text-[11px]">
                    <span class="px-2 py-0.5 bg-gray-900 text-gray-300 rounded border border-gray-800 font-mono">${sizeFormatted}</span>
                    <span class="px-2 py-0.5 bg-sky-950/60 text-sky-400 rounded border border-sky-800/40 font-mono">${iso.boot_mode}</span>
                    ${iso.arch_bios ? '<span class="px-1.5 py-0.5 bg-gray-800/80 text-gray-300 rounded">BIOS</span>' : ''}
                    ${iso.arch_uefi64 ? '<span class="px-1.5 py-0.5 bg-gray-800/80 text-gray-300 rounded">UEFI</span>' : ''}
                    ${iso.secure_boot_supported ? '<span class="px-1.5 py-0.5 bg-emerald-950 text-emerald-400 rounded border border-emerald-800/50">SecureBoot</span>' : ''}
                </div>
            </div>

            <!-- Action buttons -->
            <div class="mt-4 pt-3 border-t border-gray-800 flex items-center justify-between text-xs">
                <button onclick="setIsoAsDefault('${iso.id}')" title="Set as default boot item" class="text-gray-400 hover:text-amber-400 flex items-center space-x-1">
                    <span>${iso.is_default ? '★ Default' : '☆ Make Default'}</span>
                </button>
                <div class="flex items-center space-x-2">
                    <button onclick="openEditIsoModal('${iso.id}')" class="px-2.5 py-1 bg-gray-800 hover:bg-gray-700 text-gray-200 rounded font-medium">Edit</button>
                    <button onclick="deleteIso('${iso.id}', '${escapeHtml(iso.title)}')" class="px-2.5 py-1 bg-rose-950/40 hover:bg-rose-900/60 text-rose-400 rounded font-medium">Delete</button>
                </div>
            </div>
        `;
        grid.appendChild(card);
    });

    lucide.createIcons();
}

function setCategoryFilter(cat, elem) {
    currentCategory = cat;
    document.querySelectorAll('.cat-pill').forEach(btn => {
        btn.classList.remove('bg-sky-600', 'text-white');
        btn.classList.add('bg-gray-900', 'text-gray-400');
    });
    const target = elem || (typeof event !== 'undefined' ? (event.currentTarget || event.target) : null);
    if (target) {
        target.classList.add('bg-sky-600', 'text-white');
        target.classList.remove('bg-gray-900', 'text-gray-400');
    }
    renderIsos();
}

function filterIsos() {
    renderIsos();
}

async function toggleIsoEnabled(id, is_enabled) {
    try {
        await apiFetch(`/api/isos/${id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ is_enabled })
        });
        const iso = allIsos.find(i => i.id === id);
        if (iso) iso.is_enabled = is_enabled;
        renderIsos();
    } catch (e) {
        console.error('Failed to update enabled status:', e);
    }
}

async function setIsoAsDefault(id) {
    try {
        await apiFetch(`/api/isos/${id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ is_default: true })
        });
        allIsos.forEach(i => i.is_default = (i.id === id));
        renderIsos();
    } catch (e) {
        console.error('Failed to set default ISO:', e);
    }
}

async function deleteIso(id, title) {
    if (!confirm(`Are you sure you want to delete "${title}"? This will permanently remove the ISO file from disk.`)) {
        return;
    }
    try {
        const res = await apiFetch(`/api/isos/${id}`, { method: 'DELETE' });
        if (res.ok) {
            allIsos = allIsos.filter(i => i.id !== id);
            renderIsos();
            document.getElementById('isoCountBadge').textContent = allIsos.length;
        }
    } catch (e) {
        alert('Failed to delete ISO');
    }
}

// --- Edit ISO Modal ---
function openEditIsoModal(id) {
    const iso = allIsos.find(i => i.id === id);
    if (!iso) return;

    document.getElementById('editIsoId').value = iso.id;
    document.getElementById('editTitle').value = iso.title;
    document.getElementById('editDesc').value = iso.description || '';
    document.getElementById('editCategory').value = iso.category;
    document.getElementById('editBootMode').value = iso.boot_mode;
    document.getElementById('editCustomIpxe').value = iso.custom_ipxe || '';
    document.getElementById('editArchBios').checked = iso.arch_bios;
    document.getElementById('editArchUefi').checked = iso.arch_uefi64;
    document.getElementById('editSecureBoot').checked = iso.secure_boot_supported;
    document.getElementById('editIsDefault').checked = iso.is_default;

    document.getElementById('editIsoModal').classList.remove('hidden');
    lucide.createIcons();
}

function closeEditIsoModal() {
    document.getElementById('editIsoModal').classList.add('hidden');
}

async function submitEditIso(e) {
    e.preventDefault();
    const id = document.getElementById('editIsoId').value;
    const payload = {
        title: document.getElementById('editTitle').value.trim(),
        description: document.getElementById('editDesc').value.trim(),
        category: document.getElementById('editCategory').value,
        boot_mode: document.getElementById('editBootMode').value,
        custom_ipxe: document.getElementById('editCustomIpxe').value.trim() || null,
        arch_bios: document.getElementById('editArchBios').checked,
        arch_uefi64: document.getElementById('editArchUefi').checked,
        secure_boot_supported: document.getElementById('editSecureBoot').checked,
        is_default: document.getElementById('editIsDefault').checked,
    };

    try {
        const res = await apiFetch(`/api/isos/${id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });

        if (res.ok) {
            const updated = await res.json();
            const index = allIsos.findIndex(i => i.id === id);
            if (index !== -1) allIsos[index] = updated;
            if (updated.is_default) {
                allIsos.forEach(i => { if (i.id !== id) i.is_default = false; });
            }
            renderIsos();
            closeEditIsoModal();
        }
    } catch (e) {
        alert('Failed to save ISO changes');
    }
}

// --- Add ISO Modal (Upload / Download) ---
function openAddIsoModal(mode = 'download') {
    document.getElementById('addIsoModal').classList.remove('hidden');
    switchAddModalTab(mode);
    lucide.createIcons();
}

function closeAddIsoModal() {
    document.getElementById('addIsoModal').classList.add('hidden');
}

function switchAddModalTab(mode) {
    const dlBtn = document.getElementById('addModalTabDl');
    const upBtn = document.getElementById('addModalTabUp');
    const dlForm = document.getElementById('downloadIsoForm');
    const upForm = document.getElementById('uploadIsoForm');

    if (mode === 'download') {
        dlBtn.classList.add('bg-sky-600', 'text-white');
        dlBtn.classList.remove('text-gray-400');
        upBtn.classList.remove('bg-sky-600', 'text-white');
        upBtn.classList.add('text-gray-400');
        dlForm.classList.remove('hidden');
        upForm.classList.add('hidden');
    } else {
        upBtn.classList.add('bg-sky-600', 'text-white');
        upBtn.classList.remove('text-gray-400');
        dlBtn.classList.remove('bg-sky-600', 'text-white');
        dlBtn.classList.add('text-gray-400');
        upForm.classList.remove('hidden');
        dlForm.classList.add('hidden');
    }
}

async function loadPresets() {
    try {
        const res = await apiFetch('/api/downloads/presets');
        if (res.ok) {
            allPresets = await res.json();
            const select = document.getElementById('presetSelect');
            select.innerHTML = '<option value="">-- Choose an OS to auto-fill --</option>';
            allPresets.forEach(preset => {
                const opt = document.createElement('option');
                opt.value = preset.name;
                opt.textContent = `${preset.title} (${preset.category.toUpperCase()})`;
                select.appendChild(opt);
            });
        }
    } catch (e) {
        console.error('Failed to load presets:', e);
    }
}

function applyPreset() {
    const val = document.getElementById('presetSelect').value;
    const preset = allPresets.find(p => p.name === val);
    if (!preset) return;

    document.getElementById('dlUrl').value = preset.url;
    document.getElementById('dlTitle').value = preset.title;
    document.getElementById('dlDesc').value = preset.description;
    document.getElementById('dlCategory').value = preset.category;
    document.getElementById('dlBootMode').value = preset.boot_mode;
    document.getElementById('dlSecureBoot').checked = preset.secure_boot;
}

async function submitDownloadIso(e) {
    e.preventDefault();
    const payload = {
        url: document.getElementById('dlUrl').value.trim(),
        title: document.getElementById('dlTitle').value.trim() || undefined,
        description: document.getElementById('dlDesc').value.trim() || undefined,
        category: document.getElementById('dlCategory').value,
        boot_mode: document.getElementById('dlBootMode').value,
        secure_boot_supported: document.getElementById('dlSecureBoot').checked
    };

    try {
        const res = await apiFetch('/api/downloads', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });

        if (res.ok) {
            closeAddIsoModal();
            switchTab('downloads');
            loadDownloads();
        } else {
            alert('Failed to start download');
        }
    } catch (e) {
        alert('Network error starting download');
    }
}

function onFileSelected(input) {
    if (input.files && input.files[0]) {
        const file = input.files[0];
        document.getElementById('uploadFileLabel').textContent = `${file.name} (${formatBytes(file.size)})`;
        if (!document.getElementById('upTitle').value) {
            document.getElementById('upTitle').value = file.name.replace(/\.(iso|img)$/i, '').replace(/[-_]/g, ' ');
        }
    }
}

async function submitUploadIso(e) {
    e.preventDefault();
    const fileInput = document.getElementById('uploadFile');
    if (!fileInput.files || !fileInput.files[0]) {
        alert('Please select an ISO file');
        return;
    }

    const formData = new FormData();
    formData.append('file', fileInput.files[0]);
    formData.append('title', document.getElementById('upTitle').value.trim());
    formData.append('description', document.getElementById('upDesc').value.trim());
    formData.append('category', document.getElementById('upCategory').value);

    const progressContainer = document.getElementById('uploadProgressContainer');
    const progressBar = document.getElementById('uploadProgressBar');
    const percentText = document.getElementById('uploadPercent');
    const submitBtn = document.getElementById('uploadSubmitBtn');

    progressContainer.classList.remove('hidden');
    submitBtn.disabled = true;

    // XMLHttpRequest for real upload progress
    const xhr = new XMLHttpRequest();
    xhr.open('POST', '/api/isos/upload');
    const token = localStorage.getItem('pxe_token');
    if (token) xhr.setRequestHeader('Authorization', `Bearer ${token}`);

    xhr.upload.onprogress = (evt) => {
        if (evt.lengthComputable) {
            const percent = Math.round((evt.loaded / evt.total) * 100);
            progressBar.style.width = `${percent}%`;
            percentText.textContent = `${percent}%`;
        }
    };

    xhr.onload = async () => {
        submitBtn.disabled = false;
        progressContainer.classList.add('hidden');
        if (xhr.status === 201 || xhr.status === 200) {
            closeAddIsoModal();
            await loadIsos();
            switchTab('isos');
        } else {
            alert('Upload failed: ' + xhr.responseText);
        }
    };

    xhr.onerror = () => {
        submitBtn.disabled = false;
        progressContainer.classList.add('hidden');
        alert('Upload network error');
    };

    xhr.send(formData);
}

// --- Downloads Manager ---
async function loadDownloads() {
    try {
        const res = await apiFetch('/api/downloads');
        if (!res.ok) return;

        const tasks = await res.json();
        const activeCount = tasks.filter(t => t.status === 'downloading').length;

        // Auto-refresh ISO library when any download completes
        let newlyCompleted = false;
        tasks.forEach(t => {
            if (t.status === 'completed' && !knownCompletedDownloadIds.has(t.id)) {
                knownCompletedDownloadIds.add(t.id);
                newlyCompleted = true;
            }
        });
        if (newlyCompleted) {
            loadIsos();
        }

        const badge = document.getElementById('downloadCountBadge');
        if (activeCount > 0) {
            badge.textContent = activeCount;
            badge.classList.remove('hidden');
        } else {
            badge.classList.add('hidden');
        }

        const container = document.getElementById('downloadsList');
        if (!container) return;

        if (tasks.length === 0) {
            container.innerHTML = `
                <div class="text-center py-12 bg-cardbg rounded-xl border border-dashed border-gray-800 text-gray-400 text-sm">
                    No active or recent downloads.
                </div>
            `;
            return;
        }

        container.innerHTML = '';
        tasks.forEach(task => {
            const card = document.createElement('div');
            card.className = 'bg-cardbg rounded-xl border border-gray-800 p-4 space-y-3';

            const total = task.total_bytes || 0;
            const downloaded = task.bytes_downloaded;
            const percent = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0;
            const speedMb = (task.speed_bps / (1024 * 1024)).toFixed(1);
            const etaText = task.eta_seconds ? `${task.eta_seconds}s remaining` : 'Calculating...';

            let statusBadge = '';
            if (task.status === 'downloading') {
                statusBadge = '<span class="px-2 py-0.5 rounded text-xs bg-sky-950 text-sky-400 border border-sky-800/60 font-semibold animate-pulse">Downloading</span>';
            } else if (task.status === 'completed') {
                statusBadge = '<span class="px-2 py-0.5 rounded text-xs bg-emerald-950 text-emerald-400 border border-emerald-800/60 font-semibold">Completed</span>';
            } else if (task.status === 'cancelled') {
                statusBadge = '<span class="px-2 py-0.5 rounded text-xs bg-gray-800 text-gray-400 font-semibold">Cancelled</span>';
            } else {
                statusBadge = '<span class="px-2 py-0.5 rounded text-xs bg-rose-950 text-rose-400 border border-rose-800/60 font-semibold">Failed</span>';
            }

            card.innerHTML = `
                <div class="flex items-center justify-between">
                    <div>
                        <h4 class="text-sm font-bold text-white">${escapeHtml(task.title)}</h4>
                        <p class="text-xs font-mono text-gray-400 mt-0.5">${escapeHtml(task.filename)}</p>
                    </div>
                    <div class="flex items-center space-x-3">
                        ${statusBadge}
                        ${task.status === 'downloading' ? `
                            <button onclick="cancelDownload('${task.id}')" class="text-xs text-rose-400 hover:text-rose-300 font-semibold px-2 py-1 rounded bg-rose-950/40">
                                Cancel
                            </button>
                        ` : ''}
                    </div>
                </div>

                <!-- Progress Bar -->
                <div>
                    <div class="w-full bg-gray-900 rounded-full h-2 overflow-hidden border border-gray-800">
                        <div class="h-2 rounded-full transition-all duration-300 ${task.status === 'completed' ? 'bg-emerald-500' : task.status === 'failed' ? 'bg-rose-500' : 'bg-sky-500'}" style="width: ${task.status === 'completed' ? 100 : percent}%"></div>
                    </div>
                    <div class="flex justify-between text-xs text-gray-400 mt-1.5 font-mono">
                        <span>${formatBytes(downloaded)} / ${total > 0 ? formatBytes(total) : 'Unknown'} (${percent}%)</span>
                        ${task.status === 'downloading' ? `<span>${speedMb} MB/s • ${etaText}</span>` : ''}
                    </div>
                </div>
            `;
            container.appendChild(card);
        });

    } catch (e) {
        console.error('Failed to load downloads:', e);
    }
}

async function cancelDownload(id) {
    await apiFetch(`/api/downloads/${id}`, { method: 'DELETE' });
    loadDownloads();
}

// --- PXE Preview & Terminal Simulator ---
async function loadPxePreview() {
    try {
        const res = await apiFetch('/api/pxe/preview');
        if (res.ok) {
            const data = await res.json();
            document.getElementById('ipxeCodeViewer').textContent = data.ipxe;

            // Update CRT screen
            document.querySelectorAll('.sim-ip').forEach(el => el.textContent = data.server_ip);
            document.getElementById('rawIpxeLink').href = `http://${data.server_ip}:${data.http_port}/boot.ipxe`;

            const menuContainer = document.getElementById('simMenuItems');
            menuContainer.innerHTML = '';

            allIsos.filter(i => i.is_enabled).forEach(iso => {
                const item = document.createElement('div');
                item.className = 'flex items-center space-x-2';
                item.innerHTML = `
                    <span class="text-sky-400 font-bold">&gt;</span>
                    <span class="text-white">${escapeHtml(iso.title)}</span>
                    <span class="text-slate-500 text-[10px]">[${iso.category}]</span>
                `;
                menuContainer.appendChild(item);
            });

            const localItem = document.createElement('div');
            localItem.className = 'flex items-center space-x-2 text-slate-400 pt-2 border-t border-slate-800 mt-2';
            localItem.innerHTML = `<span class="text-emerald-400 font-bold">&gt;</span><span>Boot from Local Storage (Default)</span>`;
            menuContainer.appendChild(localItem);
        }
    } catch (e) {
        console.error('Failed to load PXE preview:', e);
    }
}

function copyIpxeUrl() {
    const rawLink = document.getElementById('rawIpxeLink').href;
    navigator.clipboard.writeText(rawLink).then(() => {
        alert('Copied iPXE Boot URL: ' + rawLink);
    });
}

// --- Client Boot Logs ---
async function loadLogs() {
    try {
        const res = await apiFetch('/api/logs?limit=100');
        if (res.ok) {
            const logs = await res.json();
            const tbody = document.getElementById('logsTableBody');
            tbody.innerHTML = '';

            if (logs.length === 0) {
                tbody.innerHTML = `<tr><td colspan="5" class="px-6 py-8 text-center text-gray-500">No client boot events recorded yet.</td></tr>`;
                return;
            }

            logs.forEach(log => {
                const tr = document.createElement('tr');
                tr.className = 'hover:bg-gray-900/50';
                tr.innerHTML = `
                    <td class="px-6 py-3 text-gray-400">${new Date(log.timestamp).toLocaleString()}</td>
                    <td class="px-6 py-3 font-semibold text-sky-400">${escapeHtml(log.client_ip)}</td>
                    <td class="px-6 py-3">
                        <span class="px-2 py-0.5 rounded bg-gray-800 text-gray-300 text-[11px]">${escapeHtml(log.boot_platform || 'PXE')}</span>
                    </td>
                    <td class="px-6 py-3 text-white">${escapeHtml(log.requested_resource)}</td>
                    <td class="px-6 py-3 text-gray-500 text-[11px] truncate max-w-xs" title="${escapeHtml(log.user_agent)}">${escapeHtml(log.user_agent)}</td>
                `;
                tbody.appendChild(tr);
            });
        }
    } catch (e) {
        console.error('Failed to load logs:', e);
    }
}

// --- User Management ---
async function loadUsers() {
    try {
        const res = await apiFetch('/api/users');
        if (res.ok) {
            const users = await res.json();
            const tbody = document.getElementById('usersTableBody');
            tbody.innerHTML = '';

            users.forEach(u => {
                const tr = document.createElement('tr');
                tr.className = 'hover:bg-gray-900/50';

                const roleBadge = u.role === 'admin'
                    ? '<span class="px-2 py-0.5 text-xs font-bold rounded bg-sky-950 text-sky-400 border border-sky-800/60">ADMIN</span>'
                    : '<span class="px-2 py-0.5 text-xs font-medium rounded bg-gray-800 text-gray-300">OPERATOR</span>';

                tr.innerHTML = `
                    <td class="px-6 py-4 font-semibold text-white flex items-center space-x-2">
                        <div class="h-7 w-7 rounded-full bg-sky-500/20 text-sky-400 flex items-center justify-center font-bold text-xs">${u.username.charAt(0).toUpperCase()}</div>
                        <span>${escapeHtml(u.username)}</span>
                    </td>
                    <td class="px-6 py-4">${roleBadge}</td>
                    <td class="px-6 py-4 text-xs text-gray-400">${new Date(u.created_at).toLocaleDateString()}</td>
                    <td class="px-6 py-4 text-right">
                        ${currentUser.role === 'admin' && u.id !== currentUser.id ? `
                            <button onclick="deleteUser(${u.id}, '${escapeHtml(u.username)}')" class="text-xs text-rose-400 hover:text-rose-300 font-semibold px-2 py-1 rounded bg-rose-950/40">
                                Delete
                            </button>
                        ` : ''}
                    </td>
                `;
                tbody.appendChild(tr);
            });
        }
    } catch (e) {
        console.error('Failed to load users:', e);
    }
}

function openCreateUserModal() {
    document.getElementById('createUserModal').classList.remove('hidden');
    lucide.createIcons();
}

function closeCreateUserModal() {
    document.getElementById('createUserModal').classList.add('hidden');
}

async function submitCreateUser(e) {
    e.preventDefault();
    const username = document.getElementById('newUsername').value.trim();
    const password = document.getElementById('newPassword').value;
    const role = document.getElementById('newRole').value;

    try {
        const res = await apiFetch('/api/users', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password, role })
        });

        if (res.ok) {
            closeCreateUserModal();
            loadUsers();
            alert(`User "${username}" created successfully!`);
        } else {
            const err = await res.json();
            alert('Error: ' + err.error);
        }
    } catch (e) {
        alert('Network error creating user');
    }
}

async function deleteUser(id, username) {
    if (!confirm(`Are you sure you want to delete user "${username}"?`)) return;

    try {
        const res = await apiFetch(`/api/users/${id}`, { method: 'DELETE' });
        if (res.ok) {
            loadUsers();
        } else {
            const err = await res.json();
            alert('Failed to delete: ' + err.error);
        }
    } catch (e) {
        alert('Network error');
    }
}

// --- Settings ---
async function loadSettings() {
    try {
        const res = await apiFetch('/api/settings');
        if (res.ok) {
            const s = await res.json();
            document.getElementById('settingServerIp').value = s.server_ip;
            document.getElementById('settingHttpPort').value = s.http_port;
            document.getElementById('settingBootTimeout').value = s.boot_timeout;
            document.getElementById('settingDefaultAction').value = s.default_boot_action;
            document.getElementById('settingProxyDhcp').checked = s.proxydhcp_enabled;
            document.getElementById('settingMenuTitle').value = s.menu_title;

            // Update header info
            document.getElementById('headerServerIp').textContent = s.server_ip;
            document.getElementById('headerProxyDhcp').textContent = s.proxydhcp_enabled ? 'Enabled' : 'Disabled';
        }
    } catch (e) {
        console.error('Failed to load settings:', e);
    }
}

async function saveSettings(e) {
    e.preventDefault();
    const payload = {
        server_ip: document.getElementById('settingServerIp').value.trim() || undefined,
        boot_timeout: parseInt(document.getElementById('settingBootTimeout').value, 10),
        default_boot_action: document.getElementById('settingDefaultAction').value,
        proxydhcp_enabled: document.getElementById('settingProxyDhcp').checked,
        menu_title: document.getElementById('settingMenuTitle').value.trim(),
    };

    try {
        const res = await apiFetch('/api/settings', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
        });

        if (res.ok) {
            await loadSettings();
            alert('Settings saved and dnsmasq configuration updated!');
        } else {
            alert('Failed to save settings');
        }
    } catch (e) {
        alert('Error saving settings');
    }
}

// --- System Telemetry ---
async function loadStats() {
    try {
        const res = await apiFetch('/api/stats');
        if (res.ok) {
            const stats = await res.json();
            document.getElementById('statCpu').textContent = `${Math.round(stats.cpu_usage)}%`;
            document.getElementById('statDisk').textContent = `${stats.disk_free_gb.toFixed(1)} GB free`;
            document.getElementById('headerServerIp').textContent = stats.server_ip;
        }
    } catch (e) {
        // Suppress polling error logs
    }
}

// Utilities
function formatBytes(bytes) {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function escapeHtml(str) {
    if (!str) return '';
    return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
}
