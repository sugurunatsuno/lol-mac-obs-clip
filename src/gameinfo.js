const { invoke } = window.__TAURI__.core;

async function refreshPlayers() {
  try {
    const players = await invoke('get_current_players');
    const list = document.getElementById('playerList');
    if (!list) return;
    list.innerHTML = '';
    players.forEach((p) => {
      const li = document.createElement('li');
      li.textContent = `${p.name} (${p.champion})`;
      list.appendChild(li);
    });
  } catch (e) {
    console.error('Failed to fetch players', e);
  }
}

window.addEventListener('DOMContentLoaded', () => {
  refreshPlayers();
  setInterval(refreshPlayers, 2000);
});
