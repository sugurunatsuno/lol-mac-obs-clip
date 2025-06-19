use crate::lol::LolEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSnapshot {
    pub game_time: f64,
    pub real_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedEvent {
    #[serde(flatten)]
    pub event: LolEvent,
    pub real_time: f64,
}

#[derive(Debug, Default, Clone)]
pub struct TimeSyncData {
    pub snapshots: Vec<TimeSnapshot>,
    pub events: Vec<TimedEvent>,
}

pub struct TimeSyncState(pub std::sync::Arc<std::sync::Mutex<TimeSyncData>>);

pub fn game_to_real_time(game_time: f64, snaps: &[TimeSnapshot]) -> Option<f64> {
    if snaps.is_empty() {
        return None;
    }
    let mut prev = &snaps[0];
    for snap in snaps.iter() {
        if snap.game_time >= game_time {
            if (snap.game_time - game_time).abs() < f64::EPSILON {
                return Some(snap.real_time);
            }
            let dt = snap.game_time - prev.game_time;
            if dt.abs() < f64::EPSILON {
                return Some(prev.real_time);
            }
            let ratio = (game_time - prev.game_time) / dt;
            return Some(prev.real_time + ratio * (snap.real_time - prev.real_time));
        }
        prev = snap;
    }
    // extrapolate using last two snapshots
    if snaps.len() < 2 {
        return Some(prev.real_time);
    }
    let last = snaps.last().unwrap();
    let prev_last = &snaps[snaps.len() - 2];
    let dt = last.game_time - prev_last.game_time;
    if dt.abs() < f64::EPSILON {
        Some(last.real_time)
    } else {
        Some(last.real_time + (game_time - last.game_time) / dt * (last.real_time - prev_last.real_time))
    }
}

pub fn real_to_game_time(real_time: f64, snaps: &[TimeSnapshot]) -> Option<f64> {
    if snaps.is_empty() {
        return None;
    }
    let mut prev = &snaps[0];
    for snap in snaps.iter() {
        if snap.real_time >= real_time {
            if (snap.real_time - real_time).abs() < f64::EPSILON {
                return Some(snap.game_time);
            }
            let dt = snap.real_time - prev.real_time;
            if dt.abs() < f64::EPSILON {
                return Some(prev.game_time);
            }
            let ratio = (real_time - prev.real_time) / dt;
            return Some(prev.game_time + ratio * (snap.game_time - prev.game_time));
        }
        prev = snap;
    }
    if snaps.len() < 2 {
        return Some(prev.game_time);
    }
    let last = snaps.last().unwrap();
    let prev_last = &snaps[snaps.len() - 2];
    let dt = last.real_time - prev_last.real_time;
    if dt.abs() < f64::EPSILON {
        Some(last.game_time)
    } else {
        Some(last.game_time + (real_time - last.real_time) / dt * (last.game_time - prev_last.game_time))
    }
}
