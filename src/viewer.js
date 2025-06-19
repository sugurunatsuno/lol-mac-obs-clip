// 動画再生ページでクリップのメタデータを表示する処理群
const { convertFileSrc, invoke } = window.__TAURI__.core;
const {} = window.__TAURI__.path;
// 動画ファイルパスの変換や Rust コマンド呼び出しに使用

// イベント種類ごとに色を決定するヘルパー
function eventColor(name) {
  switch (name) {
    case "ChampionKill":
      return "#dc3545";
    case "Multikill":
      return "#ffc107";
    case "TurretKilled":
      return "#0d6efd";
    default:
      return "#6c757d";
  }
}
// 取得したイベント名を色に変換


// イベント内容を分かりやすいテキストに変換
function formatEvent(e) {
  switch (e.EventName) {
    case 'ChampionKill':
      return `${e.KillerName ?? 'Unknown'} → ${e.VictimName ?? 'Unknown'}`;
    case 'Multikill':
      return `Multikill x${e.KillStreak ?? ''}`;
    default:
      return e.EventName;
  }
}
// イベント表示用に整形

// プレイヤー初期化とイベントメタデータの読み込み
async function init() {
  const params = new URLSearchParams(window.location.search);
  const file = params.get('file');
  if (!file) return; // パラメータが無ければ何もしない

  document.getElementById('fileName').textContent = file.split('/').pop();
  const metadataEl = document.getElementById('metadata');

  const ext = file.split('.').pop().toLowerCase();
  let type = 'video/mp4';
  if (ext === 'mkv') {
    type = 'video/x-matroska';
  } else if (ext === 'mov') {
    type = 'video/quicktime';
  }

  const player = videojs('player', {
    controls: true,
    autoplay: false,
    preload: 'auto',
    fluid: true,
    techOrder: ['html5'],
    sources: [{ src: convertFileSrc(file, 'asset'), type: type }],
  });
  let markers;
  let currentIndex = 0;
  let events = [];

  try {
    events = await invoke('get_clip_metadata', { path: file });
  } catch (e) {
    console.error('Failed to load clip metadata', e);
  }

  function addMarkers() {
    // 再生バー上にイベント位置を表示
    if (markers || events.length === 0 || !player.duration()) return;
    const holder = player.el().querySelector(".vjs-progress-holder");
    if (!holder) return;
    holder.style.position = "relative";
    markers = document.createElement("div");
    markers.className = "event-marker-container";
    holder.appendChild(markers);
    events.forEach((e) => {
      const m = document.createElement("span");
      m.className = "event-marker";
      m.style.left = `${(e.offset / player.duration()) * 100}%`;
      m.style.backgroundColor = eventColor(e.EventName);
      m.title = `${formatEvent(e)}\n${JSON.stringify(e, null, 2)}`;
      markers.appendChild(m);
    });
  } // addMarkers end


  // 再生位置に合わせて表示するイベントを更新
  function update() {
    if (events.length === 0) return;
    const t = player.currentTime();

    while (
      currentIndex < events.length - 1 &&
      events[currentIndex + 1].offset <= t
    ) {
      currentIndex++;
    }
    while (currentIndex > 0 && events[currentIndex].offset > t) {
      currentIndex--;
    }

    const current = events[currentIndex];
    if (current && t >= current.offset) {
      metadataEl.textContent = `${formatEvent(current)} (@${current.offset.toFixed(
        1
      )}s)`;
    }
  } // update end

  player.on('loadedmetadata', addMarkers); // 再生準備完了時にマーカー追加
  player.on('timeupdate', update); // 再生位置が変わるたびに呼び出し
  player.on('seeked', update); // シーク時にもイベント更新
}

// DOM 解析後に初期化処理を実行
window.addEventListener('DOMContentLoaded', init); // ページ読み込み時
