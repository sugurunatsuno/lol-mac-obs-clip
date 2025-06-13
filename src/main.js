const { invoke } = window.__TAURI__.core;

let greetInputEl;
let greetMsgEl;

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  greetMsgEl.textContent = await invoke("greet", { name: greetInputEl.value });
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document.querySelector("#greet-form").addEventListener("submit", (e) => {
    e.preventDefault();
    greet();
  });
});

window.addEventListener("DOMContentLoaded", () => {
  // 各ボタンにイベントリスナーを設定
  document.getElementById('btnStart').addEventListener('click', async () => {
    await invoke('start_recording');
    logEvent("🎥 録画を開始しました");
    updateStatus();
  });

  document.getElementById('btnStop').addEventListener('click', async () => {
    await invoke('stop_recording');
    logEvent("⏹ 録画を停止しました");
    updateStatus();
  });

  document.getElementById('btnReplayStart').addEventListener('click', async () => {
    const segmentSeconds = parseInt(document.getElementById('segmentSeconds').value, 10);
    const fps = parseInt(document.getElementById('fpsInput').value, 10);
    const videoSource = document.getElementById('videoSourceInput').value;
    const audioSource = document.getElementById('audioSourceInput').value;
    await invoke('start_ffmpeg_replay', { segmentSeconds, fps, videoSource, audioSource });
    logEvent("🔁 リプレイバッファを開始しました");
    updateStatus();
  });

  document.getElementById('btnReplayStop').addEventListener('click', async () => {
    await invoke('stop_ffmpeg_replay');
    logEvent("⏸ リプレイバッファを停止しました");
    updateStatus();
  });

  document.getElementById('btnReplaySave').addEventListener('click', async () => {
    await invoke('save_ffmpeg_clip');
    logEvent("💾 リプレイバッファを保存しました");
    updateStatus();
  });

  document.getElementById('btnGetDir').addEventListener('click', async () => {
    const dir = await invoke('get_saved_directory');
    logEvent(`📁 保存ディレクトリ: ${dir}`);
  });

  setInterval(updateStatus, 1500);
  updateStatus();
});

async function updateStatus() {
  const status = await invoke('get_status');
  document.getElementById('gameState').textContent = status.game_state;
  document.getElementById('obsState').textContent = status.obs_state;
}

function logEvent(message) {
  const log = document.getElementById('eventLog');
  const entry = document.createElement('li');
  entry.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
  log.prepend(entry);
}
