const { invoke } = window.__TAURI__.core;

async function loadVideos() {
  const list = await invoke('list_saved_videos');
  const tbody = document.getElementById('videoTableBody');
  tbody.innerHTML = '';
  list.forEach((info) => {
    const tr = document.createElement('tr');

    const nameTd = document.createElement('td');
    const link = document.createElement('a');
    link.href = `video.html?file=${encodeURIComponent(info.path)}`;
    link.textContent = info.name;
    nameTd.appendChild(link);

    const dateTd = document.createElement('td');
    dateTd.textContent = info.modified;

    const sizeTd = document.createElement('td');
    sizeTd.textContent = `${(info.size / (1024 * 1024)).toFixed(1)} MB`;

    tr.appendChild(nameTd);
    tr.appendChild(dateTd);
    tr.appendChild(sizeTd);

    tbody.appendChild(tr);
  });
}

window.addEventListener('DOMContentLoaded', loadVideos);
