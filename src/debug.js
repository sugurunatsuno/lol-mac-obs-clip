const { invoke } = window.__TAURI__.core;

async function refreshDebug() {
  try {
    const status = await invoke('get_status');
    const el = document.getElementById('debugStatus');
    if (!el) return;
    const modeText = status.recording_mode === 'Shell' ? 'ffmpeg' : 'OBS';
    const recText = status.is_recording
      ? status.replay_buffer_running
        ? 'Buffering'
        : 'Recording'
      : 'Idle';
    el.textContent = `Mode: ${modeText} | ${recText}`;
  } catch (e) {
    console.error('Failed to update debug status', e);
  }
}

window.addEventListener('DOMContentLoaded', () => {
  refreshDebug();
  setInterval(refreshDebug, 1500);
});
