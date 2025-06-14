const { invoke } = window.__TAURI__.core;

async function loadVideos() {
  const list = await invoke('list_saved_videos');
  const ul = document.getElementById('videoList');
  ul.innerHTML = '';
  list.forEach((path) => {
    const li = document.createElement('li');
    li.className = 'list-group-item';
    const a = document.createElement('a');
    a.href = `video.html?file=${encodeURIComponent(path)}`;
    a.textContent = path.split('/').pop();
    li.appendChild(a);
    ul.appendChild(li);
  });
}

window.addEventListener('DOMContentLoaded', loadVideos);
