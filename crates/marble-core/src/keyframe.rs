//! Keyframe animation executor for map object animations.

use std::collections::HashMap;

use crate::map::{EasingType, Keyframe, KeyframeSequence};

use serde::{Deserialize, Serialize};

/// Represents an active animation interpolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveAnimation {
    target_id: String,
    start_translation: [f32; 2],
    end_translation: [f32; 2],
    start_rotation: f32,
    end_rotation: f32,
    duration: f32,
    elapsed: f32,
    easing: EasingType,
}

impl ActiveAnimation {
    /// Computes the current interpolated position and rotation.
    fn interpolate(&self) -> ([f32; 2], f32) {
        let t = if self.duration > 0.0 {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let eased_t = self.easing.apply(t);

        let pos = [
            self.start_translation[0] + (self.end_translation[0] - self.start_translation[0]) * eased_t,
            self.start_translation[1] + (self.end_translation[1] - self.start_translation[1]) * eased_t,
        ];
        let rot = self.start_rotation + (self.end_rotation - self.start_rotation) * eased_t;

        (pos, rot)
    }

    /// Returns true if the animation has completed.
    fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }
}

/// Loop frame tracking for nested loops.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoopFrame {
    start_index: usize,
    remaining: Option<u32>,
}

/// Executes a keyframe animation sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyframeExecutor {
    sequence_name: String,
    current_index: usize,
    loop_stack: Vec<LoopFrame>,
    delay_remaining: f32,
    active_animations: Vec<ActiveAnimation>,
    finished: bool,
}

impl KeyframeExecutor {
    /// Creates a new executor for the given sequence name.
    pub fn new(sequence_name: String) -> Self {
        Self {
            sequence_name,
            current_index: 0,
            loop_stack: Vec::new(),
            delay_remaining: 0.0,
            active_animations: Vec::new(),
            finished: false,
        }
    }

    /// Returns the sequence name this executor is running.
    pub fn sequence_name(&self) -> &str {
        &self.sequence_name
    }

    /// Returns true if the sequence has finished executing.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Updates the executor by the given delta time.
    /// Returns a list of object updates (id, translation, rotation_radians).
    pub fn update(
        &mut self,
        dt: f32,
        sequences: &[KeyframeSequence],
        current_positions: &HashMap<String, ([f32; 2], f32)>,
        initial_transforms: &HashMap<String, ([f32; 2], f32)>,
    ) -> Vec<(String, [f32; 2], f32)> {
        if self.finished {
            return Vec::new();
        }

        // Find the sequence
        let sequence = match sequences.iter().find(|s| s.name == self.sequence_name) {
            Some(seq) => seq,
            None => {
                self.finished = true;
                return Vec::new();
            }
        };

        let mut updates = Vec::new();

        // Process delay
        if self.delay_remaining > 0.0 {
            self.delay_remaining -= dt;
            if self.delay_remaining > 0.0 {
                // Still in delay, update active animations
                return self.update_active_animations(dt);
            }
            // Delay finished, continue to next keyframe
            self.delay_remaining = 0.0;
        }

        // Update active animations
        updates.extend(self.update_active_animations(dt));

        // Check if all active animations are finished
        let all_finished = self.active_animations.iter().all(|a| a.is_finished());
        if !all_finished && !self.active_animations.is_empty() {
            return updates;
        }

        // Clear finished animations
        self.active_animations.clear();

        // Process keyframes
        while self.current_index < sequence.keyframes.len() {
            let keyframe = &sequence.keyframes[self.current_index];

            match keyframe {
                Keyframe::LoopStart { count } => {
                    self.loop_stack.push(LoopFrame {
                        start_index: self.current_index,
                        remaining: *count,
                    });
                    self.current_index += 1;
                }
                Keyframe::LoopEnd => {
                    if let Some(mut frame) = self.loop_stack.pop() {
                        let should_continue = match &mut frame.remaining {
                            Some(count) => {
                                if *count > 1 {
                                    *count -= 1;
                                    true
                                } else {
                                    false
                                }
                            }
                            None => true, // Infinite loop
                        };

                        if should_continue {
                            self.loop_stack.push(frame.clone());
                            self.current_index = frame.start_index + 1;
                        } else {
                            self.current_index += 1;
                        }
                    } else {
                        self.current_index += 1;
                    }
                }
                Keyframe::Delay { duration } => {
                    self.delay_remaining = *duration;
                    self.current_index += 1;
                    break;
                }
                Keyframe::Apply {
                    target_ids,
                    translation,
                    rotation,
                    duration,
                    easing,
                } => {
                    // Start animations for each target
                    for target_id in target_ids {
                        let current = current_positions.get(target_id);
                        let initial = initial_transforms.get(target_id);

                        if let (Some(&(cur_pos, cur_rot)), Some(&(init_pos, init_rot))) = (current, initial) {
                            let end_translation = translation
                                .map(|t| [init_pos[0] + t[0], init_pos[1] + t[1]])
                                .unwrap_or(cur_pos);

                            let end_rotation = rotation
                                .map(|r| init_rot + r.to_radians())
                                .unwrap_or(cur_rot);

                            self.active_animations.push(ActiveAnimation {
                                target_id: target_id.clone(),
                                start_translation: cur_pos,
                                end_translation,
                                start_rotation: cur_rot,
                                end_rotation,
                                duration: *duration,
                                elapsed: 0.0,
                                easing: *easing,
                            });
                        }
                    }
                    self.current_index += 1;
                    break;
                }
            }
        }

        // Check if finished
        if self.current_index >= sequence.keyframes.len()
            && self.active_animations.is_empty()
            && self.delay_remaining <= 0.0
            && self.loop_stack.is_empty()
        {
            self.finished = true;
        }

        updates
    }

    /// Updates active animations and returns position updates.
    fn update_active_animations(&mut self, dt: f32) -> Vec<(String, [f32; 2], f32)> {
        let mut updates = Vec::new();

        for anim in &mut self.active_animations {
            anim.elapsed += dt;
            let (pos, rot) = anim.interpolate();
            updates.push((anim.target_id.clone(), pos, rot));
        }

        updates
    }

    /// Resets the executor to its initial state.
    pub fn reset(&mut self) {
        self.current_index = 0;
        self.loop_stack.clear();
        self.delay_remaining = 0.0;
        self.active_animations.clear();
        self.finished = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sequence() -> KeyframeSequence {
        KeyframeSequence {
            name: "test".to_string(),
            keyframes: vec![
                Keyframe::Apply {
                    target_ids: vec!["obj1".to_string()],
                    translation: Some([100.0, 0.0]),
                    rotation: None,
                    duration: 1.0,
                    easing: EasingType::Linear,
                },
                Keyframe::Delay { duration: 0.5 },
                Keyframe::Apply {
                    target_ids: vec!["obj1".to_string()],
                    translation: Some([0.0, 0.0]),
                    rotation: None,
                    duration: 1.0,
                    easing: EasingType::Linear,
                },
            ],
            autoplay: true,
        }
    }

    fn create_loop_sequence() -> KeyframeSequence {
        KeyframeSequence {
            name: "loop_test".to_string(),
            keyframes: vec![
                Keyframe::LoopStart { count: Some(2) },
                Keyframe::Apply {
                    target_ids: vec!["obj1".to_string()],
                    translation: Some([50.0, 0.0]),
                    rotation: None,
                    duration: 0.5,
                    easing: EasingType::Linear,
                },
                Keyframe::LoopEnd,
            ],
            autoplay: true,
        }
    }

    #[test]
    fn test_executor_creation() {
        let executor = KeyframeExecutor::new("test".to_string());
        assert_eq!(executor.sequence_name(), "test");
        assert!(!executor.is_finished());
    }

    #[test]
    fn test_linear_animation() {
        let mut executor = KeyframeExecutor::new("test".to_string());
        let sequences = vec![create_test_sequence()];
        let mut positions = HashMap::new();
        let mut initials = HashMap::new();
        positions.insert("obj1".to_string(), ([0.0, 0.0], 0.0));
        initials.insert("obj1".to_string(), ([0.0, 0.0], 0.0));

        // First update starts the animation
        let updates = executor.update(0.0, &sequences, &positions, &initials);
        assert_eq!(updates.len(), 0); // No updates yet, just started

        // Simulate animation progress
        for _ in 0..10 {
            let updates = executor.update(0.1, &sequences, &positions, &initials);
            if !updates.is_empty() {
                positions.insert(updates[0].0.clone(), (updates[0].1, updates[0].2));
            }
        }

        // After 1 second, should be at [100, 0]
        let pos = positions.get("obj1").unwrap().0;
        assert!((pos[0] - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_easing_functions() {
        assert!((EasingType::Linear.apply(0.5) - 0.5).abs() < 0.001);
        assert!((EasingType::EaseIn.apply(0.5) - 0.25).abs() < 0.001);
        assert!((EasingType::EaseOut.apply(0.5) - 0.75).abs() < 0.001);

        // EaseInOut at 0.5 should be 0.5
        assert!((EasingType::EaseInOut.apply(0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_loop_execution() {
        let mut executor = KeyframeExecutor::new("loop_test".to_string());
        let sequences = vec![create_loop_sequence()];
        let mut positions = HashMap::new();
        let initials = HashMap::from([("obj1".to_string(), ([0.0, 0.0], 0.0))]);
        positions.insert("obj1".to_string(), ([0.0, 0.0], 0.0));

        // Run through the loop twice (each loop takes 0.5 seconds)
        let total_time = 2.0; // Give extra time
        let dt = 0.1;
        let steps = (total_time / dt) as u32;

        for _ in 0..steps {
            let updates = executor.update(dt, &sequences, &positions, &initials);
            for (id, pos, rot) in updates {
                positions.insert(id, (pos, rot));
            }
        }

        // After loop completes, executor should be finished
        assert!(executor.is_finished());
    }

    #[test]
    fn test_infinite_loop() {
        let sequence = KeyframeSequence {
            name: "infinite".to_string(),
            keyframes: vec![
                Keyframe::LoopStart { count: None },
                Keyframe::Apply {
                    target_ids: vec!["obj1".to_string()],
                    translation: Some([10.0, 0.0]),
                    rotation: None,
                    duration: 0.1,
                    easing: EasingType::Linear,
                },
                Keyframe::LoopEnd,
            ],
            autoplay: true,
        };

        let mut executor = KeyframeExecutor::new("infinite".to_string());
        let sequences = vec![sequence];
        let mut positions = HashMap::new();
        let initials = HashMap::from([("obj1".to_string(), ([0.0, 0.0], 0.0))]);
        positions.insert("obj1".to_string(), ([0.0, 0.0], 0.0));

        // Run for a while
        for _ in 0..100 {
            let updates = executor.update(0.05, &sequences, &positions, &initials);
            for (id, pos, rot) in updates {
                positions.insert(id, (pos, rot));
            }
        }

        // Should still be running
        assert!(!executor.is_finished());
    }
}
