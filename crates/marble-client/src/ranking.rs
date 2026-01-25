//! 실시간 순위 계산을 위한 공통 상수 및 유틸리티
//!
//! 리더보드 순위 깜빡임을 방지하기 위해 쿨타임과 히스테리시스를 적용합니다.

use marble_core::marble::PlayerId;
use std::collections::HashMap;

/// 리더보드 업데이트 쿨타임 (18프레임 = 300ms @ 60fps)
pub const LIVE_RANKING_COOLDOWN: u32 = 18;

/// 순위 변경에 필요한 최소 y좌표 차이 (히스테리시스)
pub const POSITION_CHANGE_MARGIN: f32 = 30.0;

/// 실시간 순위 추적기 (쿨타임 + 히스테리시스 적용)
pub struct LiveRankingTracker {
    /// 남은 쿨타임 프레임
    cooldown: u32,
    /// 플레이어별 마지막 y좌표
    last_positions: HashMap<PlayerId, f32>,
    /// 캐시된 순위 결과 (player_id, rank)
    cached_rankings: Vec<(PlayerId, u32)>,
}

impl Default for LiveRankingTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveRankingTracker {
    pub fn new() -> Self {
        Self {
            cooldown: 0,
            last_positions: HashMap::new(),
            cached_rankings: Vec::new(),
        }
    }

    /// 매 프레임 호출 - 쿨타임 감소
    pub fn tick(&mut self) {
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
    }

    /// 순위 업데이트 요청
    /// positions: [(player_id, y좌표), ...]
    /// 반환: 현재 유효한 순위 목록
    pub fn update(&mut self, positions: &[(PlayerId, f32)]) -> &[(PlayerId, u32)] {
        // 쿨타임 중이면 캐시된 순위 반환
        if self.cooldown > 0 {
            return &self.cached_rankings;
        }

        // 새 y좌표로 순위 계산 (낮은 y = 더 좋은 순위)
        let mut sorted_positions = positions.to_vec();
        sorted_positions.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let new_rankings: Vec<(PlayerId, u32)> = sorted_positions
            .iter()
            .enumerate()
            .map(|(idx, (pid, _))| (*pid, (idx + 1) as u32))
            .collect();

        // 변경 여부 판단: 히스테리시스 적용
        let has_significant_change = self.check_significant_change(&sorted_positions);

        if has_significant_change || self.cached_rankings.is_empty() {
            // 변경이 있으면: 쿨타임 시작 + 캐시 업데이트
            self.cooldown = LIVE_RANKING_COOLDOWN;
            self.cached_rankings = new_rankings;

            // 마지막 위치 업데이트
            self.last_positions.clear();
            for (pid, y) in positions {
                self.last_positions.insert(*pid, *y);
            }
        }

        &self.cached_rankings
    }

    /// 캐시된 순위 조회
    pub fn rankings(&self) -> &[(PlayerId, u32)] {
        &self.cached_rankings
    }

    /// 순위 변경이 유의미한지 판단 (히스테리시스)
    fn check_significant_change(&self, sorted_positions: &[(PlayerId, f32)]) -> bool {
        // 캐시가 비어있으면 변경으로 간주
        if self.cached_rankings.is_empty() {
            return true;
        }

        // 플레이어 수가 다르면 변경
        if sorted_positions.len() != self.cached_rankings.len() {
            return true;
        }

        // 새 순위와 기존 순위 비교
        for (idx, (pid, new_y)) in sorted_positions.iter().enumerate() {
            let new_rank = (idx + 1) as u32;

            // 기존 순위에서 해당 플레이어 찾기
            let old_rank = self
                .cached_rankings
                .iter()
                .find(|(p, _)| p == pid)
                .map(|(_, r)| *r);

            // 플레이어가 새로 추가됨
            let Some(old_rank) = old_rank else {
                return true;
            };

            // 순위가 변경된 경우, 히스테리시스 검사
            if new_rank != old_rank {
                // 기존 위치와 새 위치의 차이 확인
                if let Some(&old_y) = self.last_positions.get(pid) {
                    let y_diff = (new_y - old_y).abs();
                    // 30px 이상 차이나면 변경 허용
                    if y_diff >= POSITION_CHANGE_MARGIN {
                        return true;
                    }
                } else {
                    // 기존 위치 정보가 없으면 변경으로 간주
                    return true;
                }
            }
        }

        false
    }
}
