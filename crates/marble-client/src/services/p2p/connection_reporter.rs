use std::collections::HashMap;

use marble_proto::room::{PeerConnectionStatus, PeerTopology};
use matchbox_socket::PeerId;

/// RTT measurement state for a peer
#[derive(Debug, Clone)]
pub struct RttMeasurement {
    /// Last measured RTT in milliseconds
    pub rtt_ms: u32,
    /// Packet loss estimate (0.0 - 1.0)
    pub packet_loss: f32,
    /// Whether currently connected
    pub connected: bool,
    /// Pending ping timestamp (for RTT calculation)
    pub pending_ping: Option<f64>,
    /// Number of successful pings
    pub ping_count: u32,
    /// Number of failed pings (timeouts)
    pub ping_failures: u32,
}

impl Default for RttMeasurement {
    fn default() -> Self {
        Self {
            rtt_ms: 0,
            packet_loss: 0.0,
            connected: true,
            pending_ping: None,
            ping_count: 0,
            ping_failures: 0,
        }
    }
}

impl RttMeasurement {
    /// Record a successful pong response
    pub fn record_pong(&mut self, sent_timestamp: f64, received_timestamp: f64) {
        let rtt = (received_timestamp - sent_timestamp) as u32;
        // Exponential moving average
        if self.ping_count == 0 {
            self.rtt_ms = rtt;
        } else {
            self.rtt_ms = (self.rtt_ms as f32 * 0.7 + rtt as f32 * 0.3) as u32;
        }
        self.ping_count += 1;
        self.connected = true;
        self.pending_ping = None;

        // Update packet loss estimate
        self.update_packet_loss();
    }

    /// Record a ping timeout
    pub fn record_timeout(&mut self) {
        self.ping_failures += 1;
        self.pending_ping = None;
        self.update_packet_loss();

        // Mark as disconnected after too many failures
        if self.ping_failures > 5 {
            self.connected = false;
        }
    }

    /// Update packet loss estimate
    fn update_packet_loss(&mut self) {
        let total = self.ping_count + self.ping_failures;
        if total > 0 {
            self.packet_loss = self.ping_failures as f32 / total as f32;
        }
    }

    /// Reset failure count on reconnection
    pub fn mark_connected(&mut self) {
        self.connected = true;
        self.ping_failures = 0;
    }
}

/// Manages connection quality reporting to the server
pub struct ConnectionReporter {
    /// Room ID
    room_id: String,
    /// My player ID
    my_player_id: String,
    /// Peer measurements (player_id -> measurement)
    measurements: HashMap<String, RttMeasurement>,
    /// Last report timestamp
    last_report_time: f64,
    /// Report interval in milliseconds
    report_interval_ms: f64,
}

impl ConnectionReporter {
    pub fn new(room_id: String, my_player_id: String) -> Self {
        Self {
            room_id,
            my_player_id,
            measurements: HashMap::new(),
            last_report_time: 0.0,
            report_interval_ms: 5000.0, // Report every 5 seconds
        }
    }

    /// Register a peer for tracking
    pub fn add_peer(&mut self, player_id: String, _peer_id: PeerId) {
        self.measurements.insert(player_id, RttMeasurement::default());
    }

    /// Remove a peer from tracking
    pub fn remove_peer(&mut self, player_id: &str) {
        self.measurements.remove(player_id);
    }

    /// Record ping sent
    pub fn on_ping_sent(&mut self, player_id: &str, timestamp: f64) {
        if let Some(measurement) = self.measurements.get_mut(player_id) {
            measurement.pending_ping = Some(timestamp);
        }
    }

    /// Record pong received
    pub fn on_pong_received(&mut self, player_id: &str, sent_timestamp: f64, received_timestamp: f64) {
        if let Some(measurement) = self.measurements.get_mut(player_id) {
            measurement.record_pong(sent_timestamp, received_timestamp);
        }
    }

    /// Check for ping timeouts (call periodically)
    pub fn check_timeouts(&mut self, current_time: f64, timeout_ms: f64) {
        for (_player_id, measurement) in &mut self.measurements {
            if let Some(ping_time) = measurement.pending_ping {
                if current_time - ping_time > timeout_ms {
                    measurement.record_timeout();
                }
            }
        }
    }

    /// Check if it's time to send a report
    pub fn should_report(&self, current_time: f64) -> bool {
        current_time - self.last_report_time >= self.report_interval_ms
    }

    /// Generate a connection report
    pub fn generate_report(&mut self, current_time: f64) -> Vec<PeerConnectionStatus> {
        self.last_report_time = current_time;

        self.measurements
            .iter()
            .map(|(player_id, m)| PeerConnectionStatus {
                peer_id: player_id.clone(), // Using player_id as peer_id for simplicity
                rtt_ms: m.rtt_ms,
                packet_loss: m.packet_loss,
                connected: m.connected,
            })
            .collect()
    }

    /// Get room ID
    pub fn room_id(&self) -> &str {
        &self.room_id
    }

    /// Get my player ID
    pub fn my_player_id(&self) -> &str {
        &self.my_player_id
    }

    /// Mark a peer as connected
    pub fn mark_peer_connected(&mut self, player_id: &str) {
        if let Some(measurement) = self.measurements.get_mut(player_id) {
            measurement.mark_connected();
        }
    }

    /// Mark a peer as disconnected
    pub fn mark_peer_disconnected(&mut self, player_id: &str) {
        if let Some(measurement) = self.measurements.get_mut(player_id) {
            measurement.connected = false;
        }
    }

    /// Get RTT for a peer
    pub fn get_rtt(&self, player_id: &str) -> Option<u32> {
        self.measurements.get(player_id).map(|m| m.rtt_ms)
    }

    /// Get all peer statuses for UI display
    pub fn get_all_statuses(&self) -> Vec<(String, u32, f32, bool)> {
        self.measurements
            .iter()
            .map(|(pid, m)| (pid.clone(), m.rtt_ms, m.packet_loss, m.connected))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtt_measurement() {
        let mut m = RttMeasurement::default();

        // First measurement
        m.record_pong(0.0, 50.0);
        assert_eq!(m.rtt_ms, 50);
        assert_eq!(m.ping_count, 1);

        // Second measurement (should be averaged)
        m.record_pong(100.0, 170.0);
        assert!(m.rtt_ms > 50 && m.rtt_ms < 70);
    }

    #[test]
    fn test_packet_loss() {
        let mut m = RttMeasurement::default();

        m.record_pong(0.0, 50.0); // Success
        m.record_pong(100.0, 150.0); // Success
        m.record_timeout(); // Failure

        assert!(m.packet_loss > 0.0 && m.packet_loss < 0.5);
    }

    #[test]
    fn test_reporter() {
        let mut reporter = ConnectionReporter::new("room1".to_string(), "player1".to_string());
        reporter.add_peer("player2".to_string(), PeerId::new());

        // Simulate ping/pong
        reporter.on_ping_sent("player2", 0.0);
        reporter.on_pong_received("player2", 0.0, 45.0);

        let report = reporter.generate_report(5000.0);
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].rtt_ms, 45);
    }
}
