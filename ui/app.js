(() => {
  'use strict';

  const REFRESH_INTERVAL_MS = 5000;

  const tauriCore = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core : null;
  const invoke = tauriCore && typeof tauriCore.invoke === 'function' ? tauriCore.invoke : null;

  const els = {
    ramPill: document.getElementById('ram-pill'),
    windowCount: document.getElementById('window-count'),
    trimRamBtn: document.getElementById('trim-ram-btn'),
    toggleAddBtn: document.getElementById('toggle-add-btn'),
    addForm: document.getElementById('add-account-form'),
    nameInput: document.getElementById('account-name-input'),
    autoOpenInput: document.getElementById('account-auto-open'),
    cancelAddBtn: document.getElementById('cancel-add-btn'),
    submitAddBtn: document.getElementById('submit-add-btn'),
    profilesList: document.getElementById('profiles-list'),
    errorBanner: document.getElementById('error-banner'),
    errorMessage: document.getElementById('error-message'),
    errorDismiss: document.getElementById('error-dismiss')
  };

  function showError(err) {
    const message = typeof err === 'string'
      ? err
      : (err && err.message ? err.message : 'Đã xảy ra lỗi không xác định.');
    els.errorMessage.textContent = message;
    els.errorBanner.hidden = false;
  }

  function clearError() {
    els.errorBanner.hidden = true;
    els.errorMessage.textContent = '';
  }

  function formatRamMb(value) {
    if (typeof value !== 'number' || !Number.isFinite(value)) return '-- MB';
    return `${value.toFixed(1)} MB`;
  }

  function setAddFormVisible(visible) {
    els.addForm.hidden = !visible;
    els.toggleAddBtn.setAttribute('aria-expanded', visible ? 'true' : 'false');
    if (visible) {
      els.nameInput.focus();
    } else {
      els.addForm.reset();
    }
  }

  function createProfileRow(profile) {
    const row = document.createElement('div');
    row.className = 'profile-row';
    row.setAttribute('role', 'listitem');

    const info = document.createElement('div');
    info.className = 'profile-info';

    const heading = document.createElement('div');
    heading.className = 'profile-heading';

    const nameSpan = document.createElement('span');
    nameSpan.className = 'profile-name';
    nameSpan.textContent = profile.name || profile.id;

    const statusPill = document.createElement('span');
    statusPill.className = `status-pill ${profile.is_open ? 'is-open' : 'is-idle'}`;
    statusPill.textContent = profile.is_open ? 'Đang mở' : 'Sẵn sàng';

    heading.appendChild(nameSpan);
    heading.appendChild(statusPill);

    const idSpan = document.createElement('span');
    idSpan.className = 'profile-id';
    idSpan.textContent = `ID: ${profile.id}`;

    info.appendChild(heading);
    info.appendChild(idSpan);

    const controls = document.createElement('div');
    controls.className = 'profile-controls';

    const toggleLabel = document.createElement('label');
    toggleLabel.className = 'checkbox-label';

    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = Boolean(profile.auto_open);
    checkbox.setAttribute('aria-label', `Tự động mở ${profile.name || profile.id}`);
    checkbox.addEventListener('change', async () => {
      const nextValue = checkbox.checked;
      checkbox.disabled = true;
      try {
        clearError();
        await invoke('cmd_toggle_auto_open', { id: profile.id, autoOpen: nextValue });
        await refreshAll();
      } catch (err) {
        checkbox.checked = !nextValue;
        showError(err);
      } finally {
        checkbox.disabled = false;
      }
    });

    const toggleText = document.createElement('span');
    toggleText.textContent = 'Tự mở';

    toggleLabel.appendChild(checkbox);
    toggleLabel.appendChild(toggleText);

    const openBtn = document.createElement('button');
    openBtn.type = 'button';
    openBtn.className = 'btn btn-primary';
    openBtn.textContent = profile.is_open ? 'Hiện cửa sổ' : 'Mở Zalo';
    openBtn.addEventListener('click', async () => {
      openBtn.disabled = true;
      try {
        clearError();
        await invoke('cmd_open_profile', { id: profile.id });
        await refreshAll();
      } catch (err) {
        showError(err);
      } finally {
        openBtn.disabled = false;
      }
    });

    controls.appendChild(toggleLabel);
    controls.appendChild(openBtn);

    row.appendChild(info);
    row.appendChild(controls);
    return row;
  }

  function renderProfiles(profiles) {
    els.profilesList.textContent = '';

    if (!Array.isArray(profiles) || profiles.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'empty-state';
      empty.textContent = 'Chưa có tài khoản nào. Nhấn "Thêm tài khoản" để tạo hồ sơ mới.';
      els.profilesList.appendChild(empty);
      return;
    }

    const frag = document.createDocumentFragment();
    for (const profile of profiles) {
      frag.appendChild(createProfileRow(profile));
    }
    els.profilesList.appendChild(frag);
  }

  async function refreshAll() {
    if (!invoke) {
      showError('Không tìm thấy Tauri IPC (window.__TAURI__.core.invoke).');
      return;
    }

    try {
      const [profiles, stats] = await Promise.all([
        invoke('cmd_list_profiles'),
        invoke('cmd_runtime_stats')
      ]);

      renderProfiles(profiles);

      if (stats && typeof stats === 'object') {
        els.ramPill.textContent = formatRamMb(stats.ram_mb);
        const openCount = typeof stats.open_windows === 'number' ? stats.open_windows : 0;
        els.windowCount.textContent = `${openCount} cửa sổ đang chạy`;
      }
    } catch (err) {
      showError(err);
    }
  }

  function bindEvents() {
    els.errorDismiss.addEventListener('click', clearError);

    els.toggleAddBtn.addEventListener('click', () => {
      setAddFormVisible(els.addForm.hidden);
    });

    els.cancelAddBtn.addEventListener('click', () => {
      setAddFormVisible(false);
    });

    els.trimRamBtn.addEventListener('click', async () => {
      if (!invoke) return;
      els.trimRamBtn.disabled = true;
      try {
        clearError();
        const trimmedMb = await invoke('cmd_trim_ram');
        if (typeof trimmedMb === 'number') {
          els.ramPill.textContent = formatRamMb(trimmedMb);
        }
        await refreshAll();
      } catch (err) {
        showError(err);
      } finally {
        els.trimRamBtn.disabled = false;
      }
    });

    els.addForm.addEventListener('submit', async (event) => {
      event.preventDefault();
      if (!invoke) return;

      const name = els.nameInput.value.trim();
      if (!name) {
        showError('Vui lòng nhập tên tài khoản.');
        els.nameInput.focus();
        return;
      }

      const autoOpen = Boolean(els.autoOpenInput.checked);
      els.submitAddBtn.disabled = true;
      try {
        clearError();
        await invoke('cmd_create_profile', { name, autoOpen });
        setAddFormVisible(false);
        await refreshAll();
      } catch (err) {
        showError(err);
      } finally {
        els.submitAddBtn.disabled = false;
      }
    });
  }

  document.addEventListener('DOMContentLoaded', () => {
    bindEvents();
    refreshAll();
    setInterval(refreshAll, REFRESH_INTERVAL_MS);
  });
})();
