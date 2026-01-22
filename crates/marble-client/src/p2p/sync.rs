//! Synchronization and desync detection.
//!
//! Handles frame hash exchange, desync detection, and state resynchronization.

use matchbox_socket::PeerId;
use std::collections::HashMap;

/// Interval (in frames) at which to exchange frame hashes.
pub const HASH_EXCHANGE_INTERVAL: u64 = 60;

/// Number of consecutive mismatches to trigger resync.
pub const DESYNC_THRESHOLD: u32 = 2;

/// Ping interval in milliseconds.
pub const PING_INTERVAL_MS: u32 = 1000;

/// Hash comparison result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashCompareResult {
    /// All hashes match.
    Match,
    /// Waiting for more peers to report.
    Waiting,
    /// Desync detected with the majority hash.
    Desync { majority_hash: u64 },
}

/// Synchronization state tracker.
#[derive(Debug, Clone, Default)]
pub struct SyncTracker {
    /// Collected frame hashes: frame -> (peer_id -> hash)
    frame_hashes: HashMap<u64, HashMap<PeerId, u64>>,
    /// Number of consecutive desync detections.
    consecutive_desyncs: u32,
    /// Last frame we sent our hash for.
    last_hash_frame: u64,
}

impl SyncTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a hash received from a peer.
    pub fn record_peer_hash(&mut self, frame: u64, peer_id: PeerId, hash: u64) {
        self.frame_hashes
            .entry(frame)
            .or_default()
            .insert(peer_id, hash);
    }

    /// Check if we should send a hash for the given frame.
    pub fn should_send_hash(&self, current_frame: u64) -> bool {
        current_frame > 0
            && current_frame % HASH_EXCHANGE_INTERVAL == 0
            && current_frame != self.last_hash_frame
    }

    /// Mark that we sent a hash for a frame.
    pub fn mark_hash_sent(&mut self, frame: u64) {
        self.last_hash_frame = frame;
    }

    /// Compare our hash with peers at a given frame.
    pub fn compare_hashes(
        &mut self,
        frame: u64,
        my_hash: u64,
        expected_peer_count: usize,
    ) -> HashCompareResult {
        let Some(peer_hashes) = self.frame_hashes.get(&frame) else {
            return HashCompareResult::Waiting;
        };

        // Wait until we have hashes from all peers
        if peer_hashes.len() < expected_peer_count {
            return HashCompareResult::Waiting;
        }

        // Count occurrences of each hash
        let mut hash_counts: HashMap<u64, usize> = HashMap::new();
        *hash_counts.entry(my_hash).or_insert(0) += 1;

        for hash in peer_hashes.values() {
            *hash_counts.entry(*hash).or_insert(0) += 1;
        }

        // Find the majority hash
        let (&majority_hash, &count) = hash_counts.iter().max_by_key(|&(_, c)| *c).unwrap();
        let total = expected_peer_count + 1; // +1 for self

        // If all hashes match
        if count == total {
            self.consecutive_desyncs = 0;
            self.cleanup_old_frames(frame);
            return HashCompareResult::Match;
        }

        // Desync detected
        self.consecutive_desyncs += 1;

        // Only report desync if my hash doesn't match the majority
        if my_hash != majority_hash {
            HashCompareResult::Desync { majority_hash }
        } else {
            // My hash is correct, but someone else desynced
            // They will request sync from me
            HashCompareResult::Match
        }
    }

    /// Check if we've hit the desync threshold.
    pub fn should_request_resync(&self) -> bool {
        self.consecutive_desyncs >= DESYNC_THRESHOLD
    }

    /// Reset the desync counter.
    pub fn reset_desync_counter(&mut self) {
        self.consecutive_desyncs = 0;
    }

    /// Clean up old frame data.
    fn cleanup_old_frames(&mut self, current_frame: u64) {
        // Keep only the last few frames of data
        let min_frame = current_frame.saturating_sub(HASH_EXCHANGE_INTERVAL * 5);
        self.frame_hashes.retain(|&f, _| f >= min_frame);
    }

    /// Clear all tracking data.
    pub fn clear(&mut self) {
        self.frame_hashes.clear();
        self.consecutive_desyncs = 0;
        self.last_hash_frame = 0;
    }
}

/// RTT (Round Trip Time) tracker for ping/pong.
#[derive(Debug, Clone, Default)]
pub struct RttTracker {
    /// Pending pings: peer_id -> sent timestamp.
    pending_pings: HashMap<PeerId, f64>,
    /// Last measured RTT per peer.
    rtts: HashMap<PeerId, u32>,
    /// Last ping sent time (global).
    last_ping_time: f64,
}

impl RttTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if we should send a ping.
    pub fn should_ping(&self, now: f64) -> bool {
        now - self.last_ping_time >= PING_INTERVAL_MS as f64
    }

    /// Record that we sent a ping.
    pub fn record_ping_sent(&mut self, peer_id: PeerId, timestamp: f64) {
        self.pending_pings.insert(peer_id, timestamp);
        self.last_ping_time = timestamp;
    }

    /// Process a pong and return the RTT in milliseconds.
    pub fn process_pong(&mut self, peer_id: PeerId, sent_timestamp: f64, now: f64) -> Option<u32> {
        // Check if this matches a pending ping
        if let Some(&pending_ts) = self.pending_pings.get(&peer_id) {
            // Use the timestamp from the pong message to calculate RTT
            if (pending_ts - sent_timestamp).abs() < 1.0 {
                self.pending_pings.remove(&peer_id);
                let rtt = (now - sent_timestamp) as u32;
                self.rtts.insert(peer_id, rtt);
                return Some(rtt);
            }
        }

        // Fallback: just calculate RTT from the sent timestamp
        let rtt = (now - sent_timestamp) as u32;
        self.rtts.insert(peer_id, rtt);
        Some(rtt)
    }

    /// Get the RTT for a peer.
    pub fn get_rtt(&self, peer_id: PeerId) -> Option<u32> {
        self.rtts.get(&peer_id).copied()
    }

    /// Get all RTTs.
    pub fn all_rtts(&self) -> &HashMap<PeerId, u32> {
        &self.rtts
    }

    /// Remove a peer from tracking.
    pub fn remove_peer(&mut self, peer_id: PeerId) {
        self.pending_pings.remove(&peer_id);
        self.rtts.remove(&peer_id);
    }

    /// Clear all tracking data.
    pub fn clear(&mut self) {
        self.pending_pings.clear();
        self.rtts.clear();
        self.last_ping_time = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_peer_id(n: u128) -> PeerId {
        PeerId(Uuid::from_u128(n))
    }

    #[test]
    fn test_hash_match() {
        let mut tracker = SyncTracker::new();
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let frame = 60;
        let hash = 0xABCD1234;

        tracker.record_peer_hash(frame, peer1, hash);
        tracker.record_peer_hash(frame, peer2, hash);

        let result = tracker.compare_hashes(frame, hash, 2);
        assert_eq!(result, HashCompareResult::Match);
    }

    #[test]
    fn test_hash_desync() {
        let mut tracker = SyncTracker::new();
        let peer1 = make_peer_id(1);
        let peer2 = make_peer_id(2);

        let frame = 60;
        let good_hash = 0xABCD1234;
        let bad_hash = 0xDEADBEEF;

        tracker.record_peer_hash(frame, peer1, good_hash);
        tracker.record_peer_hash(frame, peer2, good_hash);

        // My hash is different from the majority
        let result = tracker.compare_hashes(frame, bad_hash, 2);
        assert!(matches!(result, HashCompareResult::Desync { majority_hash } if majority_hash == good_hash));
    }

    #[test]
    fn test_should_send_hash() {
        let tracker = SyncTracker::new();

        assert!(!tracker.should_send_hash(0));
        assert!(!tracker.should_send_hash(30));
        assert!(tracker.should_send_hash(60));
        assert!(tracker.should_send_hash(120));
    }

    #[test]
    fn test_rtt_tracker() {
        let mut tracker = RttTracker::new();
        let peer = make_peer_id(1);

        let sent_time = 1000.0;
        tracker.record_ping_sent(peer, sent_time);

        let now = 1050.0;
        let rtt = tracker.process_pong(peer, sent_time, now);

        assert_eq!(rtt, Some(50));
        assert_eq!(tracker.get_rtt(peer), Some(50));
    }
}
