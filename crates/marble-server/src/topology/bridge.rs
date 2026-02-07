use std::collections::HashMap;

/// Player connection quality score
#[derive(Debug, Clone, Default)]
pub struct ConnectionQuality {
    /// Average RTT in milliseconds
    pub avg_rtt_ms: f32,
    /// Packet loss rate (0.0 ~ 1.0)
    pub packet_loss: f32,
    /// Connection stability (number of successful reports)
    pub stability_score: u32,
}

impl ConnectionQuality {
    /// Calculate overall quality score (higher is better)
    pub fn score(&self) -> f32 {
        // Lower RTT and packet loss = better score
        // Higher stability = better score
        let rtt_score = 1000.0 / (self.avg_rtt_ms + 1.0);
        let loss_score = 1.0 - self.packet_loss;
        let stability = self.stability_score.min(100) as f32 / 100.0;

        rtt_score * loss_score * (0.5 + 0.5 * stability)
    }

    /// Update with new measurement
    pub fn update(&mut self, rtt_ms: u32, packet_loss: f32, connected: bool) {
        // Exponential moving average for RTT
        self.avg_rtt_ms = self.avg_rtt_ms * 0.7 + rtt_ms as f32 * 0.3;

        // Exponential moving average for packet loss
        self.packet_loss = self.packet_loss * 0.7 + packet_loss * 0.3;

        // Increment stability if connected
        if connected {
            self.stability_score = self.stability_score.saturating_add(1);
        } else {
            self.stability_score = self.stability_score.saturating_sub(5);
        }
    }
}

/// Bridge node selector based on connection quality
#[derive(Debug, Clone)]
pub struct BridgeSelector {
    /// Number of bridges per group
    pub bridges_per_group: usize,
    /// Player quality scores (player_id -> quality)
    pub player_qualities: HashMap<String, ConnectionQuality>,
}

impl BridgeSelector {
    pub fn new(bridges_per_group: usize) -> Self {
        Self {
            bridges_per_group,
            player_qualities: HashMap::new(),
        }
    }

    /// Update player's connection quality
    pub fn update_quality(
        &mut self,
        player_id: &str,
        peer_id: &str,
        rtt_ms: u32,
        packet_loss: f32,
        connected: bool,
    ) {
        let quality = self
            .player_qualities
            .entry(player_id.to_string())
            .or_default();
        quality.update(rtt_ms, packet_loss, connected);
    }

    /// Select best bridge candidates from a group
    pub fn select_bridges(&self, player_ids: &[String]) -> Vec<String> {
        let mut scored_players: Vec<(String, f32)> = player_ids
            .iter()
            .map(|pid| {
                let score = self
                    .player_qualities
                    .get(pid)
                    .map(|q| q.score())
                    .unwrap_or(0.5); // Default score for new players
                (pid.clone(), score)
            })
            .collect();

        // Sort by score descending
        scored_players.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top N as bridges
        scored_players
            .into_iter()
            .take(self.bridges_per_group)
            .map(|(pid, _)| pid)
            .collect()
    }

    /// Remove player quality data
    pub fn remove_player(&mut self, player_id: &str) {
        self.player_qualities.remove(player_id);
    }

    /// Get player's current quality score
    pub fn get_quality(&self, player_id: &str) -> Option<&ConnectionQuality> {
        self.player_qualities.get(player_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_quality_score() {
        let mut q = ConnectionQuality::default();
        q.avg_rtt_ms = 50.0;
        q.packet_loss = 0.1;
        q.stability_score = 50;

        let score = q.score();
        assert!(score > 0.0);
    }

    #[test]
    fn test_bridge_selection() {
        let mut selector = BridgeSelector::new(2);

        // Add some players with different qualities
        selector.update_quality("p1", "peer1", 100, 0.1, true);
        selector.update_quality("p2", "peer2", 50, 0.0, true);
        selector.update_quality("p3", "peer3", 200, 0.2, true);

        // Update multiple times to build stability
        for _ in 0..10 {
            selector.update_quality("p2", "peer2", 50, 0.0, true);
        }

        let bridges =
            selector.select_bridges(&["p1".to_string(), "p2".to_string(), "p3".to_string()]);
        assert_eq!(bridges.len(), 2);
        // p2 should be first due to best RTT and stability
        assert!(bridges.contains(&"p2".to_string()));
    }
}
