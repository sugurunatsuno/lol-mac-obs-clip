const { convertFileSrc, invoke } = window.__TAURI__.core;
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

function init() {
  const params = new URLSearchParams(window.location.search);
  const file = params.get('file');
  if (!file) return;

  document.getElementById('fileName').textContent = file.split('/').pop();
  const metadataEl = document.getElementById('metadata');

  const ext = file.split('.').pop().toLowerCase();
  let type = 'video/mp4';
  if (ext === 'mkv') {
    type = 'video/x-matroska';
  } else if (ext === 'mov') {
    type = 'video/quicktime';
  }

  const player = videojs('player');
  player.src({ src: convertFileSrc(file), type });
  let markers;

  function addMarkers() {
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
      m.title = formatEvent(e);
      markers.appendChild(m);
    });
  }


  let events = [];
  invoke('get_clip_metadata', { path: file })
    .then((data) => {
      if (Array.isArray(data)) {
        events = data;
      }
      addMarkers();
    })
    .catch(() => {
      /* ignore errors */
    });

  function update() {
    if (events.length === 0) return;
    const t = player.currentTime();
    let current = null;
    for (const e of events) {
      if (e.offset <= t) {
        current = e;
      } else {
        break;
      }
    }
    if (current) {
      metadataEl.textContent = `${formatEvent(current)} (@${current.offset.toFixed(
        1
      )}s)`;
    }
  }

  player.on('loadedmetadata', addMarkers);
  player.on('timeupdate', update);
  player.on('seeked', update);
}

window.addEventListener('DOMContentLoaded', init);
