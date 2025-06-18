// Rust 側との通信に使用する invoke を取得
const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

// input 要素から整数値を取得し、最小値チェックを行うユーティリティ
function getInt(id, min = 1) {
  const el = document.getElementById(id);
  if (!el) return NaN;
  const val = parseInt(el.value, 10);
  if (Number.isNaN(val) || val < min) {
    throw new Error(`${id} must be an integer >= ${min}`);
  }
  return val; // チェックを通過した値を返す
}

// DOM が読み込まれたタイミングで初期化処理を実行
window.addEventListener("DOMContentLoaded", async () => {
  if (document.getElementById('videoSourceInput')) {
    await loadDevices(); // ffmpeg 用のデバイス一覧を取得
  }
  await loadSettings(); // 保存済み設定を画面へ反映
  // 各ボタンにイベントリスナーを設定
  document.getElementById('modeToggle')?.addEventListener('change', async (e) => {
    const mode = e.target.checked ? 'Shell' : 'Obs';
    await invoke('set_recording_mode', { mode });
    updateStatus();
  });
  document.getElementById('btnStart')?.addEventListener('click', async () => {
    try {
      await invoke('start_recording');
      logEvent("🎥 録画を開始しました");
    } catch (e) {
      logEvent(`⚠️ 録画開始に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnStop')?.addEventListener('click', async () => {
    try {
      await invoke('stop_recording');
      logEvent("⏹ 録画を停止しました");
    } catch (e) {
      logEvent(`⚠️ 録画停止に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnReplayStart')?.addEventListener('click', async () => {
    const isShell = document.getElementById('modeToggle').checked;
    if (isShell) {
      try {
        const segmentSeconds = getInt('segmentSeconds');
        const fps = getInt('fpsInput');
        const videoSource = document.getElementById('videoSourceInput').value;
        const audioSource = document.getElementById('audioSourceInput').value;
        const wrapCount = getInt('wrapCountInput');
        const bitrate = document.getElementById('bitrateInput').value;
        await invoke('start_ffmpeg_replay', { segmentSeconds, fps, videoSource, audioSource, wrapCount, bitrate });
        logEvent("🔁 リプレイバッファを開始しました");
      } catch (e) {
        alert(e.message);
        logEvent(`⚠️ リプレイバッファの開始に失敗しました: ${e}`);
        return;
      }
    } else {
      try {
        await invoke('start_replay_buffer');
        logEvent("🔁 リプレイバッファを開始しました");
      } catch (e) {
        logEvent(`⚠️ リプレイバッファの開始に失敗しました: ${e}`);
      }
    }
    updateStatus();
  });

  document.getElementById('btnReplayStop')?.addEventListener('click', async () => {
    const isShell = document.getElementById('modeToggle').checked;
    try {
      if (isShell) {
        await invoke('stop_ffmpeg_replay');
      } else {
        await invoke('stop_replay_buffer');
      }
      logEvent("⏸ リプレイバッファを停止しました");
    } catch (e) {
      logEvent(`⚠️ リプレイバッファの停止に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnReplaySave')?.addEventListener('click', async () => {
    const isShell = document.getElementById('modeToggle').checked;
    try {
      if (isShell) {
        await invoke('save_ffmpeg_clip');
      } else {
        await invoke('save_replay_buffer');
      }
      logEvent("💾 リプレイバッファを保存しました");
    } catch (e) {
      logEvent(`⚠️ リプレイバッファの保存に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('chooseDirBtn')?.addEventListener('click', async () => {
    const selected = await open({
      directory: true,});
    if (selected) {
      document.getElementById('saveDirInput').value = selected;
      await invoke('set_saved_directory', { dir: selected });
    }
  });

  document.getElementById('btnGetDir')?.addEventListener('click', async () => {
    const dir = await invoke('get_saved_directory');
    await open(dir);
  });

  document.getElementById('btnSaveSettings')?.addEventListener('click', async () => {
    await saveSettings();
  });

  setInterval(updateStatus, 1500);
  updateStatus();
});

// 画面上の状態表示を最新に更新する
async function updateStatus() {
  const status = await invoke('get_status');
  document.getElementById('gameState').textContent = status.game_state;
  document.getElementById('obsState').textContent = status.obs_state;
  const toggle = document.getElementById('modeToggle');
  if (toggle) {
    toggle.checked = status.recording_mode === 'Shell';
    toggle.disabled = status.is_recording;
  }
  const saveBtn = document.getElementById('btnReplaySave');
  if (saveBtn) {
    saveBtn.disabled = !status.replay_buffer_running;
  }
}

// 画面のログ欄へテキストを追加
function logEvent(message) {
  const log = document.getElementById('eventLog');
  const entry = document.createElement('li');
  entry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
  log.prepend(entry);
}

// 設定ファイルを読み込んでフォームに反映
async function loadSettings() {
  // Rust から現在の設定を取得
  const s = await invoke('load_settings_cmd');
  const seg = document.getElementById('segmentSeconds');
  if (seg) seg.value = s.segment_seconds;
  const fps = document.getElementById('fpsInput');
  if (fps) fps.value = s.fps;
  const vsrc = document.getElementById('videoSourceInput');
  if (vsrc) vsrc.value = s.video_source;
  const asrc = document.getElementById('audioSourceInput');
  if (asrc) asrc.value = s.audio_source;
  const wrap = document.getElementById('wrapCountInput');
  if (wrap) wrap.value = s.wrap_count;
  const br = document.getElementById('bitrateInput');
  if (br) br.value = s.bitrate;
  const toggle = document.getElementById('modeToggle');
  if (toggle) toggle.checked = s.recording_mode === 'Shell';
  const dir = document.getElementById('saveDirInput');
  if (dir) dir.value = s.save_dir;
}

// ffmpeg が利用可能なデバイス一覧を取得してセレクトボックスへ展開
async function loadDevices() {
  const vSel = document.getElementById('videoSourceInput');
  const aSel = document.getElementById('audioSourceInput');
  if (!vSel || !aSel) return;
  try {
    const list = await invoke('list_ffmpeg_devices');
    vSel.innerHTML = '';
    list.video.forEach((d) => {
      const opt = document.createElement('option');
      opt.value = d.index;
      opt.textContent = `[${d.index}] ${d.name}`;
      vSel.appendChild(opt);
    });
    aSel.innerHTML = '';
    list.audio.forEach((d) => {
      const opt = document.createElement('option');
      opt.value = d.index;
      opt.textContent = `[${d.index}] ${d.name}`;
      aSel.appendChild(opt);
    });
  } catch (e) {
    logEvent(`⚠️ デバイス情報の取得に失敗しました: ${e}`);
  }
}

// 画面で指定した値を設定ファイルへ保存
async function saveSettings() {
  // 既存設定をベースに変更点だけ上書き
  const base = await invoke('load_settings_cmd');
  const settings = { ...base };
  try {
    settings.segment_seconds = getInt('segmentSeconds');
    settings.fps = getInt('fpsInput');
    const wrap = getInt('wrapCountInput');
    settings.wrap_count = wrap;
  } catch (e) {
    alert(e.message);
    return;
  }
  const vsrc = document.getElementById('videoSourceInput');
  if (vsrc) settings.video_source = vsrc.value;
  const asrc = document.getElementById('audioSourceInput');
  if (asrc) settings.audio_source = asrc.value;
  const br = document.getElementById('bitrateInput');
  if (br) settings.bitrate = br.value;
  const toggle = document.getElementById('modeToggle');
  if (toggle) settings.recording_mode = toggle.checked ? 'Shell' : 'Obs';
  const dir = document.getElementById('saveDirInput');
  if (dir) settings.save_dir = dir.value;
  await invoke('save_settings_cmd', { settings });
  await invoke('set_saved_directory', { dir: settings.save_dir });
  logEvent('⚙️ 設定を保存しました');
}
