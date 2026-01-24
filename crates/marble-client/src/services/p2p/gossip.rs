use std::collections::HashSet;

use marble_proto::play::P2pMessage;
use matchbox_socket::PeerId;

/// Wrapper for gossip message handling
pub struct GossipMessage {
    pub message: P2pMessage,
    pub from_peer: PeerId,
}

/// Gossip message handler for P2P message relay
pub struct GossipHandler {
    /// Seen message IDs for deduplication
    seen_messages: HashSet<String>,
    /// Maximum seen messages cache size
    max_cache_size: usize,
    /// My mesh group ID
    my_group: u32,
    /// Whether this node is a bridge
    is_bridge: bool,
    /// Connected peers in the same group
    group_peers: Vec<PeerId>,
    /// Bridge peers from other groups (only for bridge nodes)
    bridge_peers: Vec<PeerId>,
}

impl GossipHandler {
    pub fn new(my_group: u32, is_bridge: bool) -> Self {
        Self {
            seen_messages: HashSet::new(),
            max_cache_size: 10000,
            my_group,
            is_bridge,
            group_peers: Vec::new(),
            bridge_peers: Vec::new(),
        }
    }

    /// Update peer lists
    pub fn set_peers(&mut self, group_peers: Vec<PeerId>, bridge_peers: Vec<PeerId>) {
        self.group_peers = group_peers;
        self.bridge_peers = bridge_peers;
    }

    /// Update bridge status
    pub fn set_bridge_status(&mut self, is_bridge: bool) {
        self.is_bridge = is_bridge;
    }

    /// Check if message was already seen (for deduplication)
    pub fn is_seen(&self, message_id: &str) -> bool {
        self.seen_messages.contains(message_id)
    }

    /// Mark message as seen
    pub fn mark_seen(&mut self, message_id: String) {
        // Evict old entries if cache is full
        if self.seen_messages.len() >= self.max_cache_size {
            // Simple eviction: clear half the cache
            // In production, use LRU or time-based expiration
            let to_remove: Vec<_> = self
                .seen_messages
                .iter()
                .take(self.max_cache_size / 2)
                .cloned()
                .collect();
            for id in to_remove {
                self.seen_messages.remove(&id);
            }
        }
        self.seen_messages.insert(message_id);
    }

    /// Process an incoming message and determine relay targets
    /// Returns (should_process, relay_targets)
    pub fn handle_incoming(
        &mut self,
        msg: &P2pMessage,
        from_peer: PeerId,
    ) -> (bool, Vec<PeerId>) {
        // Check for duplicate
        if self.is_seen(&msg.message_id) {
            return (false, vec![]);
        }

        // Mark as seen
        self.mark_seen(msg.message_id.clone());

        // Check TTL
        if msg.ttl == 0 {
            return (true, vec![]);
        }

        // Determine relay targets based on role and message origin
        let relay_targets = self.get_relay_targets(msg.origin_group, from_peer);

        (true, relay_targets)
    }

    /// Get peers to relay message to
    fn get_relay_targets(&self, origin_group: u32, exclude_peer: PeerId) -> Vec<PeerId> {
        let mut targets: Vec<PeerId> = Vec::new();

        if self.is_bridge && origin_group != self.my_group {
            // Message from another group via bridge
            // Only relay to local group (don't send back to bridges)
            for peer in &self.group_peers {
                if *peer != exclude_peer {
                    targets.push(*peer);
                }
            }
        } else if self.is_bridge && origin_group == self.my_group {
            // Message from my group, I'm a bridge
            // Relay to both local group and other bridges
            for peer in &self.group_peers {
                if *peer != exclude_peer {
                    targets.push(*peer);
                }
            }
            for peer in &self.bridge_peers {
                if *peer != exclude_peer {
                    targets.push(*peer);
                }
            }
        } else {
            // Normal node: relay to connected group peers
            for peer in &self.group_peers {
                if *peer != exclude_peer {
                    targets.push(*peer);
                }
            }
        }

        targets
    }

    /// Prepare message for relay (decrement TTL)
    pub fn prepare_for_relay(&self, msg: &P2pMessage) -> P2pMessage {
        P2pMessage {
            message_id: msg.message_id.clone(),
            ttl: msg.ttl.saturating_sub(1),
            origin_group: msg.origin_group,
            origin_player: msg.origin_player.clone(),
            payload: msg.payload.clone(),
        }
    }

    /// Create a new outgoing message
    pub fn create_message(
        &mut self,
        player_id: &str,
        ttl: u32,
        payload: marble_proto::play::p2p_message::Payload,
    ) -> P2pMessage {
        let message_id = uuid::Uuid::new_v4().to_string();
        self.mark_seen(message_id.clone());

        P2pMessage {
            message_id,
            ttl,
            origin_group: self.my_group,
            origin_player: player_id.to_string(),
            payload: Some(payload),
        }
    }

    /// Get all peers (for broadcasting own messages)
    pub fn get_all_peers(&self) -> Vec<PeerId> {
        let mut all: Vec<PeerId> = self.group_peers.clone();
        if self.is_bridge {
            all.extend(self.bridge_peers.iter().copied());
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deduplication() {
        let mut handler = GossipHandler::new(0, false);

        let msg_id = "test-123".to_string();
        assert!(!handler.is_seen(&msg_id));

        handler.mark_seen(msg_id.clone());
        assert!(handler.is_seen(&msg_id));
    }

    #[test]
    fn test_ttl_check() {
        let mut handler = GossipHandler::new(0, false);

        let msg = P2pMessage {
            message_id: "test".to_string(),
            ttl: 0,
            origin_group: 0,
            origin_player: "p1".to_string(),
            payload: None,
        };

        let peer = PeerId::new();
        let (should_process, targets) = handler.handle_incoming(&msg, peer);

        assert!(should_process);
        assert!(targets.is_empty()); // TTL 0 means no relay
    }
}
