//! Keyframe animation executor for map object animations.

use std::collections::HashMap;

use crate::dsl::GameContext;
use crate::map::{EasingType, Keyframe, KeyframeSequence, PivotMode, RollDirection};

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
    /// Optional pivot point for pivot rotation (flipper-style).
    /// When set, the object rotates around this point.
    pivot: Option<[f32; 2]>,
    /// Initial offset from pivot (calculated once at animation start).
    initial_pivot_offset: Option<[f32; 2]>,
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

        // Check if this is a pivot rotation
        if let (Some(pivot), Some(offset)) = (self.pivot, self.initial_pivot_offset) {
            // Pivot rotation: rotate the offset vector around the pivot
            let angle = self.start_rotation + (self.end_rotation - self.start_rotation) * eased_t;
            let (sin, cos) = angle.sin_cos();

            // Rotate the initial offset
            let rotated_x = offset[0] * cos - offset[1] * sin;
            let rotated_y = offset[0] * sin + offset[1] * cos;

            // Calculate new position
            let pos = [pivot[0] + rotated_x, pivot[1] + rotated_y];

            (pos, angle)
        } else {
            // Standard translation + rotation interpolation
            let pos = [
                self.start_translation[0]
                    + (self.end_translation[0] - self.start_translation[0]) * eased_t,
                self.start_translation[1]
                    + (self.end_translation[1] - self.start_translation[1]) * eased_t,
            ];
            let rot = self.start_rotation + (self.end_rotation - self.start_rotation) * eased_t;

            (pos, rot)
        }
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

    /// Returns the current keyframe index being executed.
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Updates the executor by the given delta time.
    /// Returns a list of object updates (id, translation, rotation_radians).
    pub fn update(
        &mut self,
        dt: f32,
        sequences: &[KeyframeSequence],
        current_positions: &HashMap<String, ([f32; 2], f32)>,
        initial_transforms: &HashMap<String, ([f32; 2], f32)>,
        game_context: &mut GameContext,
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
                    self.delay_remaining = duration.evaluate_with_random(game_context);
                    self.current_index += 1;
                    break;
                }
                Keyframe::Apply {
                    translation,
                    rotation,
                    duration,
                    easing,
                } => {
                    // Start animations for each target (using sequence-level target_ids)
                    for target_id in &sequence.target_ids {
                        let current = current_positions.get(target_id);
                        let initial = initial_transforms.get(target_id);

                        if let (Some(&(cur_pos, cur_rot)), Some(&(init_pos, init_rot))) =
                            (current, initial)
                        {
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
                                pivot: None,
                                initial_pivot_offset: None,
                            });
                        }
                    }
                    self.current_index += 1;
                    break;
                }
                Keyframe::PivotRotate {
                    pivot,
                    pivot_mode,
                    angle,
                    duration,
                    easing,
                } => {
                    // Start pivot rotation animations for each target (using sequence-level target_ids)
                    for target_id in &sequence.target_ids {
                        let current = current_positions.get(target_id);
                        let initial = initial_transforms.get(target_id);

                        if let (Some(&(cur_pos, cur_rot)), Some(&(init_pos, init_rot))) =
                            (current, initial)
                        {
                            // Calculate world pivot and offset based on pivot mode
                            let (world_pivot, offset, end_rotation) = match pivot_mode {
                                PivotMode::Absolute => {
                                    // Absolute mode: pivot is in world coordinates
                                    // angle is relative to initial rotation
                                    // Convert world offset to local coordinates (remove initial rotation)
                                    let world_offset =
                                        [init_pos[0] - pivot[0], init_pos[1] - pivot[1]];
                                    let (sin_init, cos_init) = (-init_rot).sin_cos();
                                    let offset = [
                                        world_offset[0] * cos_init - world_offset[1] * sin_init,
                                        world_offset[0] * sin_init + world_offset[1] * cos_init,
                                    ];
                                    let end_rot = init_rot + angle.to_radians();
                                    (*pivot, offset, end_rot)
                                }
                                PivotMode::Relative => {
                                    // Relative mode: pivot is offset from current position in local coordinates
                                    // angle is relative to current rotation
                                    // Rotate pivot offset by current rotation to get world coordinates
                                    let (sin, cos) = cur_rot.sin_cos();
                                    let rotated_pivot_x = pivot[0] * cos - pivot[1] * sin;
                                    let rotated_pivot_y = pivot[0] * sin + pivot[1] * cos;
                                    let world_pivot = [
                                        cur_pos[0] + rotated_pivot_x,
                                        cur_pos[1] + rotated_pivot_y,
                                    ];
                                    // offset is in local coordinates (-pivot)
                                    let offset = [-pivot[0], -pivot[1]];
                                    let end_rot = cur_rot + angle.to_radians();
                                    (world_pivot, offset, end_rot)
                                }
                            };

                            self.active_animations.push(ActiveAnimation {
                                target_id: target_id.clone(),
                                start_translation: cur_pos,
                                end_translation: cur_pos, // Will be calculated in interpolate()
                                start_rotation: cur_rot,
                                end_rotation,
                                duration: *duration,
                                elapsed: 0.0,
                                easing: *easing,
                                pivot: Some(world_pivot),
                                initial_pivot_offset: Some(offset),
                            });
                        }
                    }
                    self.current_index += 1;
                    break;
                }
                Keyframe::ContinuousRotate { speed, direction } => {
                    // Continuous rotation - apply rotation delta each frame.
                    // Must break after applying to prevent infinite loop within a single frame.
                    let direction_mult = match direction {
                        RollDirection::Clockwise => 1.0,
                        RollDirection::Counterclockwise => -1.0,
                    };

                    for target_id in &sequence.target_ids {
                        if let Some(&(cur_pos, cur_rot)) = current_positions.get(target_id) {
                            let delta_rot = speed.to_radians() * dt * direction_mult;
                            updates.push((target_id.clone(), cur_pos, cur_rot + delta_rot));
                        }
                    }

                    self.current_index += 1;
                    break; // Process LoopEnd on next frame to avoid infinite loop
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

    /// Fast-forwards the executor to a specific keyframe index.
    ///
    /// This method calculates the cumulative transform state at the given keyframe index
    /// without animating through all intermediate states. It processes keyframes 0 through
    /// `target_index` instantly, computing the final position/rotation for each target.
    ///
    /// After calling this method, the executor is positioned at `target_index` and ready
    /// to begin smooth animation of that keyframe.
    ///
    /// Returns the calculated transforms at that state: (object_id -> (position, rotation)).
    pub fn fast_forward_to(
        &mut self,
        target_index: usize,
        sequences: &[KeyframeSequence],
        initial_transforms: &HashMap<String, ([f32; 2], f32)>,
    ) -> HashMap<String, ([f32; 2], f32)> {
        // Reset executor state
        self.reset();

        // Find the sequence
        let Some(sequence) = sequences.iter().find(|s| s.name == self.sequence_name) else {
            return initial_transforms.clone();
        };

        // Initialize state with initial transforms for all targets
        let mut state: HashMap<String, ([f32; 2], f32)> = HashMap::new();
        for target_id in &sequence.target_ids {
            if let Some(&(pos, rot)) = initial_transforms.get(target_id) {
                state.insert(target_id.clone(), (pos, rot));
            }
        }

        // Process keyframes 0..target_index to calculate cumulative state
        // (without animation - instant application)
        for (idx, keyframe) in sequence.keyframes.iter().enumerate() {
            if idx > target_index {
                break;
            }

            match keyframe {
                Keyframe::Apply {
                    translation,
                    rotation,
                    ..
                } => {
                    // Apply translation and rotation to all targets
                    for target_id in &sequence.target_ids {
                        if let Some(&(init_pos, init_rot)) = initial_transforms.get(target_id) {
                            if let Some((pos, rot)) = state.get_mut(target_id) {
                                if let Some(t) = translation {
                                    // Translation is relative to initial position
                                    pos[0] = init_pos[0] + t[0];
                                    pos[1] = init_pos[1] + t[1];
                                }
                                if let Some(r) = rotation {
                                    // Rotation is relative to initial rotation
                                    *rot = init_rot + r.to_radians();
                                }
                            }
                        }
                    }
                }
                Keyframe::PivotRotate {
                    pivot,
                    pivot_mode,
                    angle,
                    ..
                } => {
                    // Apply pivot rotation to all targets
                    for target_id in &sequence.target_ids {
                        if let Some(&(init_pos, init_rot)) = initial_transforms.get(target_id) {
                            // Get current state (result of previous keyframes) or use initial
                            let (cur_pos, _cur_rot) = state
                                .get(target_id)
                                .copied()
                                .unwrap_or((init_pos, init_rot));

                            // Calculate based on pivot mode
                            let (world_pivot, offset, final_rot) = match pivot_mode {
                                PivotMode::Absolute => {
                                    // Absolute mode: use initial position for offset calculation
                                    // Convert world offset to local coordinates (remove initial rotation)
                                    let world_offset =
                                        [init_pos[0] - pivot[0], init_pos[1] - pivot[1]];
                                    let (sin_init, cos_init) = (-init_rot).sin_cos();
                                    let offset = [
                                        world_offset[0] * cos_init - world_offset[1] * sin_init,
                                        world_offset[0] * sin_init + world_offset[1] * cos_init,
                                    ];
                                    let final_rot = init_rot + angle.to_radians();
                                    (*pivot, offset, final_rot)
                                }
                                PivotMode::Relative => {
                                    // Relative mode: pivot is offset from current position in local coordinates
                                    // Get current rotation from state
                                    let cur_rot =
                                        state.get(target_id).map(|(_, r)| *r).unwrap_or(init_rot);
                                    // Rotate pivot offset by current rotation to get world coordinates
                                    let (sin, cos) = cur_rot.sin_cos();
                                    let rotated_pivot_x = pivot[0] * cos - pivot[1] * sin;
                                    let rotated_pivot_y = pivot[0] * sin + pivot[1] * cos;
                                    let world_pivot = [
                                        cur_pos[0] + rotated_pivot_x,
                                        cur_pos[1] + rotated_pivot_y,
                                    ];
                                    // offset is in local coordinates (-pivot)
                                    let offset = [-pivot[0], -pivot[1]];
                                    let final_rot = cur_rot + angle.to_radians();
                                    (world_pivot, offset, final_rot)
                                }
                            };

                            // Apply rotation to offset
                            let (sin, cos) = final_rot.sin_cos();
                            let rotated_x = offset[0] * cos - offset[1] * sin;
                            let rotated_y = offset[0] * sin + offset[1] * cos;

                            if let Some((pos, rot)) = state.get_mut(target_id) {
                                pos[0] = world_pivot[0] + rotated_x;
                                pos[1] = world_pivot[1] + rotated_y;
                                *rot = final_rot;
                            }
                        }
                    }
                }
                // ContinuousRotate, LoopStart, LoopEnd, Delay don't affect static preview state
                Keyframe::ContinuousRotate { .. }
                | Keyframe::LoopStart { .. }
                | Keyframe::LoopEnd
                | Keyframe::Delay { .. } => {}
            }
        }

        // Set current_index to target_index so executor starts animation from there
        self.current_index = target_index;

        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::NumberOrExpr;

    fn create_test_sequence() -> KeyframeSequence {
        KeyframeSequence {
            name: "test".to_string(),
            target_ids: vec!["obj1".to_string()],
            keyframes: vec![
                Keyframe::Apply {
                    translation: Some([1.0, 0.0]),
                    rotation: None,
                    duration: 1.0,
                    easing: EasingType::Linear,
                },
                Keyframe::Delay {
                    duration: NumberOrExpr::Number(0.5),
                },
                Keyframe::Apply {
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
            target_ids: vec!["obj1".to_string()],
            keyframes: vec![
                Keyframe::LoopStart { count: Some(2) },
                Keyframe::Apply {
                    translation: Some([0.5, 0.0]),
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
        let mut game_context = GameContext::with_cache_and_seed(12345);
        positions.insert("obj1".to_string(), ([0.0, 0.0], 0.0));
        let initials = HashMap::from([("obj1".to_string(), ([0.0, 0.0], 0.0))]);

        // First update starts the animation
        let updates = executor.update(0.0, &sequences, &positions, &initials, &mut game_context);
        assert_eq!(updates.len(), 0); // No updates yet, just started

        // Simulate animation progress
        for _ in 0..10 {
            let updates =
                executor.update(0.1, &sequences, &positions, &initials, &mut game_context);
            if !updates.is_empty() {
                positions.insert(updates[0].0.clone(), (updates[0].1, updates[0].2));
            }
        }

        // After 1 second, should be at [1.0, 0]
        let pos = positions.get("obj1").unwrap().0;
        assert!((pos[0] - 1.0).abs() < 0.01);
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
        let mut game_context = GameContext::with_cache_and_seed(12345);
        positions.insert("obj1".to_string(), ([0.0, 0.0], 0.0));

        // Run through the loop twice (each loop takes 0.5 seconds)
        let total_time = 2.0; // Give extra time
        let dt = 0.1;
        let steps = (total_time / dt) as u32;

        for _ in 0..steps {
            let updates = executor.update(dt, &sequences, &positions, &initials, &mut game_context);
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
            target_ids: vec!["obj1".to_string()],
            keyframes: vec![
                Keyframe::LoopStart { count: None },
                Keyframe::Apply {
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
        let mut game_context = GameContext::with_cache_and_seed(12345);
        positions.insert("obj1".to_string(), ([0.0, 0.0], 0.0));

        // Run for a while
        for _ in 0..100 {
            let updates =
                executor.update(0.05, &sequences, &positions, &initials, &mut game_context);
            for (id, pos, rot) in updates {
                positions.insert(id, (pos, rot));
            }
        }

        // Should still be running
        assert!(!executor.is_finished());
    }

    #[test]
    fn test_pivot_rotation() {
        // Test that pivot rotation correctly rotates around the pivot point
        let sequence = KeyframeSequence {
            name: "pivot_test".to_string(),
            target_ids: vec!["flipper".to_string()],
            keyframes: vec![Keyframe::PivotRotate {
                pivot: [0.0, 0.0], // Pivot at origin
                angle: 90.0,       // Rotate 90 degrees
                duration: 1.0,
                easing: EasingType::Linear,
            }],
            autoplay: true,
        };

        let mut executor = KeyframeExecutor::new("pivot_test".to_string());
        let sequences = vec![sequence];

        // Object starts at (1.0, 0) with 0 rotation
        let mut positions = HashMap::from([("flipper".to_string(), ([1.0, 0.0], 0.0))]);
        let initials = HashMap::from([("flipper".to_string(), ([1.0, 0.0], 0.0))]);
        let mut game_context = GameContext::with_cache_and_seed(12345);

        // First update starts the animation
        executor.update(0.0, &sequences, &positions, &initials, &mut game_context);

        // Run animation to completion
        for _ in 0..10 {
            let updates =
                executor.update(0.1, &sequences, &positions, &initials, &mut game_context);
            for (id, pos, rot) in updates {
                positions.insert(id, (pos, rot));
            }
        }

        // After 90 degree rotation around origin, (1.0, 0) should be at approximately (0, 1.0)
        let (pos, rot) = positions.get("flipper").unwrap();
        assert!(pos[0].abs() < 0.01, "X should be near 0, got {}", pos[0]);
        assert!(
            (pos[1] - 1.0).abs() < 0.01,
            "Y should be near 1.0, got {}",
            pos[1]
        );
        assert!(
            (*rot - std::f32::consts::FRAC_PI_2).abs() < 0.01,
            "Rotation should be 90 degrees"
        );
    }

    #[test]
    fn test_random_delay() {
        // Test that random() in delay works
        let sequence = KeyframeSequence {
            name: "random_delay_test".to_string(),
            target_ids: vec!["obj1".to_string()],
            keyframes: vec![
                Keyframe::LoopStart { count: Some(3) },
                Keyframe::Delay {
                    duration: NumberOrExpr::Expr("random(0.1, 0.2)".to_string()),
                },
                Keyframe::Apply {
                    translation: Some([10.0, 0.0]),
                    rotation: None,
                    duration: 0.1,
                    easing: EasingType::Linear,
                },
                Keyframe::LoopEnd,
            ],
            autoplay: true,
        };

        let mut executor = KeyframeExecutor::new("random_delay_test".to_string());
        let sequences = vec![sequence];
        let mut positions = HashMap::from([("obj1".to_string(), ([0.0, 0.0], 0.0))]);
        let initials = HashMap::from([("obj1".to_string(), ([0.0, 0.0], 0.0))]);
        let mut game_context = GameContext::with_cache_and_seed(42);

        // Run for a while - the test just verifies it doesn't crash
        for _ in 0..100 {
            let updates =
                executor.update(0.05, &sequences, &positions, &initials, &mut game_context);
            for (id, pos, rot) in updates {
                positions.insert(id, (pos, rot));
            }
        }
    }
}
