const { invoke } = window.__TAURI__.core;

window.addEventListener("DOMContentLoaded", async () => {
  await loadDevices();
  await loadSettings();
  // 各ボタンにイベントリスナーを設定
  document.getElementById('modeToggle').addEventListener('change', async (e) => {
    const mode = e.target.checked ? 'Shell' : 'Obs';
    await invoke('set_recording_mode', { mode });
    updateStatus();
  });
  document.getElementById('btnStart').addEventListener('click', async () => {
    try {
      await invoke('start_recording');
      logEvent("🎥 録画を開始しました");
    } catch (e) {
      logEvent(`⚠️ 録画開始に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnStop').addEventListener('click', async () => {
    try {
      await invoke('stop_recording');
      logEvent("⏹ 録画を停止しました");
    } catch (e) {
      logEvent(`⚠️ 録画停止に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnReplayStart').addEventListener('click', async () => {
    const segmentSeconds = parseInt(document.getElementById('segmentSeconds').value, 10);
    const fps = parseInt(document.getElementById('fpsInput').value, 10);
    const videoSource = document.getElementById('videoSourceInput').value;
    const audioSource = document.getElementById('audioSourceInput').value;
    const wrapCount = parseInt(document.getElementById('wrapCountInput').value, 10);
    const bitrate = document.getElementById('bitrateInput').value;
    try {
      await invoke('start_ffmpeg_replay', { segmentSeconds, fps, videoSource, audioSource, wrapCount, bitrate });
      logEvent("🔁 リプレイバッファを開始しました");
    } catch (e) {
      logEvent(`⚠️ リプレイバッファの開始に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnReplayStop').addEventListener('click', async () => {
    try {
      await invoke('stop_ffmpeg_replay');
      logEvent("⏸ リプレイバッファを停止しました");
    } catch (e) {
      logEvent(`⚠️ リプレイバッファの停止に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnReplaySave').addEventListener('click', async () => {
    try {
      await invoke('save_ffmpeg_clip');
      logEvent("💾 リプレイバッファを保存しました");
    } catch (e) {
      logEvent(`⚠️ リプレイバッファの保存に失敗しました: ${e}`);
    }
    updateStatus();
  });

  document.getElementById('btnGetDir').addEventListener('click', async () => {
    const dir = await invoke('get_saved_directory');
    logEvent(`📁 保存ディレクトリ: ${dir}`);
  });

  document.getElementById('btnSaveSettings').addEventListener('click', async () => {
    await saveSettings();
  });

  setInterval(updateStatus, 1500);
  updateStatus();
});

async function updateStatus() {
  const status = await invoke('get_status');
  document.getElementById('gameState').textContent = status.game_state;
  document.getElementById('obsState').textContent = status.obs_state;
  const toggle = document.getElementById('modeToggle');
  toggle.checked = status.recording_mode === 'Shell';
  toggle.disabled = status.is_recording;
  const saveBtn = document.getElementById('btnReplaySave');
  saveBtn.disabled = !status.replay_buffer_running;
}

function logEvent(message) {
  const log = document.getElementById('eventLog');
  const entry = document.createElement('li');
  entry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
  log.prepend(entry);
}

async function loadSettings() {
  const s = await invoke('load_settings_cmd');
  document.getElementById('segmentSeconds').value = s.segment_seconds;
  document.getElementById('fpsInput').value = s.fps;
  document.getElementById('videoSourceInput').value = s.video_source;
  document.getElementById('audioSourceInput').value = s.audio_source;
  document.getElementById('wrapCountInput').value = s.wrap_count;
  document.getElementById('bitrateInput').value = s.bitrate;
  document.getElementById('modeToggle').checked = s.recording_mode === 'Shell';
}

async function loadDevices() {
  try {
    const list = await invoke('list_ffmpeg_devices');
    const vSel = document.getElementById('videoSourceInput');
    const aSel = document.getElementById('audioSourceInput');
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

async function saveSettings() {
  const settings = {
    segment_seconds: parseInt(document.getElementById('segmentSeconds').value, 10),
    fps: parseInt(document.getElementById('fpsInput').value, 10),
    video_source: document.getElementById('videoSourceInput').value,
    audio_source: document.getElementById('audioSourceInput').value,
    wrap_count: parseInt(document.getElementById('wrapCountInput').value, 10),
    bitrate: document.getElementById('bitrateInput').value,
    recording_mode: document.getElementById('modeToggle').checked ? 'Shell' : 'Obs'
  };
  await invoke('save_settings_cmd', { settings });
  logEvent('⚙️ 設定を保存しました');
}
