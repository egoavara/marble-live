//! 스냅 시스템
//!
//! - 격자 스냅: Global/Local 축 기준, 공유 interval
//! - Guideline 스냅: Shift 누르면 가장 가까운 guideline에 스냅 (각 guideline별 interval)

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// 스냅 설정
#[derive(Resource, Debug, Clone)]
pub struct SnapConfig {
    /// 격자 스냅 간격 (Global/Local 공유)
    pub grid_interval: f32,
    /// 각도 스냅 간격 (도 단위)
    pub angle_interval: f32,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            grid_interval: 0.05,
            angle_interval: 0.5,
        }
    }
}

/// 스냅 결과
#[derive(Debug, Clone)]
pub struct SnapResult {
    pub position: Vec2,
    /// Shift+드래그로 guideline에 스냅된 경우 해당 guideline ID
    pub snapped_guideline_id: Option<String>,
}

/// SnapManager - 스냅 연산 통합 인터페이스
///
/// Query로 GuidelineMarker를 자동으로 가져옵니다.
#[derive(SystemParam)]
pub struct SnapManager<'w, 's> {
    config: Res<'w, SnapConfig>,
    guidelines: Query<
        'w,
        's,
        (
            &'static crate::bevy::MapObjectMarker,
            &'static crate::bevy::GuidelineMarker,
        ),
    >,
}

impl<'w, 's> SnapManager<'w, 's> {
    /// Global 축 기준 스냅
    ///
    /// - 항상: 격자 스냅 (grid_interval)
    /// - Shift: 가장 가까운 guideline에 스냅
    pub fn snap(&self, pos: Vec2, shift: bool, exclude_id: Option<&String>) -> SnapResult {
        self.snap_internal(pos, Vec2::ZERO, 0.0, shift, exclude_id)
    }

    /// Local 축 기준 스냅
    ///
    /// - 항상: 로컬 좌표계 기준 격자 스냅
    /// - Shift: 가장 가까운 guideline에 스냅
    pub fn snap_local(
        &self,
        pos: Vec2,
        origin: Vec2,
        rotation: f32,
        shift: bool,
        exclude_id: Option<&String>,
    ) -> SnapResult {
        self.snap_internal(pos, origin, rotation, shift, exclude_id)
    }

    fn snap_internal(
        &self,
        pos: Vec2,
        origin: Vec2,
        rotation: f32,
        shift: bool,
        exclude_id: Option<&String>,
    ) -> SnapResult {
        // 1. 격자 스냅
        let grid_snapped = self.apply_grid_snap(pos, origin, rotation);

        // 2. Shift 안 누름 → 격자 스냅만
        if !shift {
            return SnapResult {
                position: grid_snapped,
                snapped_guideline_id: None,
            };
        }

        // 3. Shift 누름 → 가장 가까운 guideline에 스냅
        let mut best_dist = f32::MAX;
        let mut best_snap: Option<(Vec2, String)> = None;

        for (marker, guideline) in self.guidelines.iter() {
            // 자기 자신 제외
            if let Some(exclude) = exclude_id {
                if marker.object_id.as_ref() == Some(exclude) {
                    continue;
                }
            }

            // 스냅 비활성화된 guideline 제외
            if !guideline.snap_enabled {
                continue;
            }

            let dist = perpendicular_distance(grid_snapped, guideline.start, guideline.end);
            if dist < best_dist {
                best_dist = dist;
                let snapped = snap_to_line_interval(
                    grid_snapped,
                    guideline.start,
                    guideline.end,
                    guideline.ruler_interval,
                );
                best_snap = Some((snapped, marker.object_id.clone().unwrap_or_default()));
            }
        }

        match best_snap {
            Some((snapped_pos, guideline_id)) => SnapResult {
                position: snapped_pos,
                snapped_guideline_id: Some(guideline_id),
            },
            None => SnapResult {
                position: grid_snapped,
                snapped_guideline_id: None,
            },
        }
    }

    fn apply_grid_snap(&self, pos: Vec2, origin: Vec2, rotation: f32) -> Vec2 {
        let interval = self.config.grid_interval;
        if interval <= 0.0 {
            return pos;
        }

        if rotation.abs() < 0.001 && origin == Vec2::ZERO {
            // Global: 단순 격자 스냅
            Vec2::new(
                (pos.x / interval).round() * interval,
                (pos.y / interval).round() * interval,
            )
        } else {
            // Local: 로컬 좌표로 변환 → 스냅 → 월드로 복원
            let rot = Rot2::radians(-rotation);
            let local = rot * (pos - origin);
            let snapped_local = Vec2::new(
                (local.x / interval).round() * interval,
                (local.y / interval).round() * interval,
            );
            let rot_back = Rot2::radians(rotation);
            origin + rot_back * snapped_local
        }
    }

    /// 스칼라 격자 스냅 (크기, 반지름 등)
    pub fn snap_scalar(&self, value: f32) -> f32 {
        let interval = self.config.grid_interval;
        if interval <= 0.0 {
            return value;
        }
        (value / interval).round() * interval
    }

    /// 각도 스냅 (라디안 입력, 라디안 출력)
    pub fn snap_angle(&self, angle_rad: f32) -> f32 {
        let interval = self.config.angle_interval;
        if interval <= 0.0 {
            return angle_rad;
        }
        let deg = angle_rad.to_degrees();
        let snapped_deg = (deg / interval).round() * interval;
        snapped_deg.to_radians()
    }

    /// 설정 참조
    pub fn config(&self) -> &SnapConfig {
        &self.config
    }
}

/// 점에서 직선까지의 수직 거리 (무한 직선)
fn perpendicular_distance(point: Vec2, line_start: Vec2, line_end: Vec2) -> f32 {
    let line = line_end - line_start;
    let len = line.length();
    if len < 0.0001 {
        return point.distance(line_start);
    }
    let cross = (point.x - line_start.x) * line.y - (point.y - line_start.y) * line.x;
    cross.abs() / len
}

/// 직선에 투영 후 interval 단위로 스냅
fn snap_to_line_interval(point: Vec2, line_start: Vec2, line_end: Vec2, interval: f32) -> Vec2 {
    let line = line_end - line_start;
    let len = line.length();
    if len < 0.0001 {
        return line_start;
    }

    let dir = line / len;

    // 직선에 투영
    let projected_dist = (point - line_start).dot(dir);

    // interval 단위로 스냅
    let snapped_dist = if interval > 0.0 {
        (projected_dist / interval).round() * interval
    } else {
        projected_dist
    };

    line_start + dir * snapped_dist
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perpendicular_distance() {
        // 수평선 Y=0
        let dist = perpendicular_distance(Vec2::new(5.0, 3.0), Vec2::ZERO, Vec2::new(10.0, 0.0));
        assert!((dist - 3.0).abs() < 0.001);

        // 수직선 X=3
        let dist = perpendicular_distance(
            Vec2::new(5.0, 4.0),
            Vec2::new(3.0, 0.0),
            Vec2::new(3.0, 10.0),
        );
        assert!((dist - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_snap_to_line_interval() {
        // 수평선 Y=0, interval=1.0
        let snapped =
            snap_to_line_interval(Vec2::new(2.3, 0.5), Vec2::ZERO, Vec2::new(10.0, 0.0), 1.0);
        assert!((snapped.x - 2.0).abs() < 0.001);
        assert!(snapped.y.abs() < 0.001);
    }
}
