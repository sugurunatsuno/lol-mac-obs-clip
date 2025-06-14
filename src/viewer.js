const { convertFileSrc } = window.__TAURI__.core;

function init() {
  const params = new URLSearchParams(window.location.search);
  const file = params.get('file');
  if (!file) return;

  document.getElementById('fileName').textContent = file.split('/').pop();

  const ext = file.split('.').pop().toLowerCase();
  let type = 'video/mp4';
  if (ext === 'mkv') {
    type = 'video/x-matroska';
  } else if (ext === 'mov') {
    type = 'video/quicktime';
  }

  const player = videojs('player');
  player.src({ src: convertFileSrc(file), type });
}

window.addEventListener('DOMContentLoaded', init);
