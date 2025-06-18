// 保存済み動画一覧を取得しテーブル表示するスクリプト
const { invoke } = window.__TAURI__.core;

// テーブルへ動画情報を動的に展開
async function loadVideos() {
  // Rust 側から動画一覧を取得
  const list = await invoke('list_saved_videos');
  const tbody = document.getElementById('videoTableBody');
  tbody.innerHTML = '';
  list.forEach((info) => {
    const tr = document.createElement('tr');
    tr.className = 'border-b border-gray-200';

    const nameTd = document.createElement('td');
    nameTd.className = 'p-2 text-left';
    const link = document.createElement('a');
    link.href = `video.html?file=${encodeURIComponent(info.path)}`;
    link.textContent = info.name;
    nameTd.appendChild(link);

    const dateTd = document.createElement('td');
    dateTd.className = 'p-2 text-right';
    dateTd.textContent = info.modified;

    const sizeTd = document.createElement('td');
    sizeTd.className = 'p-2 text-right';
    sizeTd.textContent = `${(info.size / (1024 * 1024)).toFixed(1)} MB`;

    tr.appendChild(nameTd);
    tr.appendChild(dateTd);
    tr.appendChild(sizeTd);

    tbody.appendChild(tr);
  });
}

// ページ表示後に動画一覧を読み込む
window.addEventListener('DOMContentLoaded', loadVideos);
