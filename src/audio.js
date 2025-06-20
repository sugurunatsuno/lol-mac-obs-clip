export function playAudio(path) {
  const url = window.__TAURI__.core.convertFileSrc(path);
  const audio = new Audio(url);
  audio.play().catch((e) => console.error('Audio play failed', e));
}

window.addEventListener('DOMContentLoaded', () => {
  window.__TAURI__.event.listen('play-sound', (e) => {
    if (typeof e.payload === 'string') {
      playAudio(e.payload);
    }
  });
});
