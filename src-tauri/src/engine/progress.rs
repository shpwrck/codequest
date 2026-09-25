use super::*;

pub(super) const SCORE_RUNE_THRESHOLDS: [u32; 3] = [300, 900, 1_800];
pub(super) const DATA_CHARGE_THRESHOLDS: [u32; 3] = [3, 6, 9];
pub(super) const BUG_BREACH_THRESHOLDS: [u32; 3] = [1, 3, 5];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PresentationTier {
    Initiate,
    Adept,
    OracleBound,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum InsightStage {
    Unlit,
    RuneOne,
    RuneTwo,
    RuneThree,
}

impl InsightStage {
    pub(super) fn from_score(score: u32) -> Self {
        match threshold_stage(score, &SCORE_RUNE_THRESHOLDS) {
            0 => Self::Unlit,
            1 => Self::RuneOne,
            2 => Self::RuneTwo,
            _ => Self::RuneThree,
        }
    }

    pub(super) fn index(self) -> usize {
        self as usize
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Unlit => "UNLIT",
            Self::RuneOne => "I",
            Self::RuneTwo => "II",
            Self::RuneThree => "III",
        }
    }

    pub(super) fn color(self) -> Color {
        match self {
            Self::Unlit => MIST,
            Self::RuneOne => CYAN,
            Self::RuneTwo => AMBER,
            Self::RuneThree => MAGENTA,
        }
    }
}

pub(super) fn threshold_stage(value: u32, thresholds: &[u32]) -> usize {
    thresholds.partition_point(|threshold| value >= *threshold)
}

pub(super) fn streak_multiplier(streak: u32) -> u32 {
    match streak {
        0..=2 => 1,
        3..=5 => 2,
        _ => 3,
    }
}

pub(super) fn score_award_for_streak(streak: u32) -> u32 {
    100 * streak_multiplier(streak)
}

impl PresentationTier {
    pub(super) fn from_level(level: u32) -> Self {
        match level {
            0 | 1 => Self::Initiate,
            2 | 3 => Self::Adept,
            _ => Self::OracleBound,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Initiate => "INITIATE",
            Self::Adept => "ADEPT",
            Self::OracleBound => "ORACLE-BOUND",
        }
    }
}

/// `1ST TRY a/b`, capped so the worst case is `1ST TRY 99/99`.
pub(super) fn first_try_label((right, attempted): (u32, u32)) -> String {
    format!("1ST TRY {}/{}", right.min(99), attempted.min(99))
}

/// The roman numeral of a lit mastery-rune stage.
pub(super) fn rune_numeral(stage: usize) -> &'static str {
    match stage {
        0 => "-",
        1 => "I",
        2 => "II",
        _ => "III",
    }
}

/// Roman numeral for a lit mastery stage (1-3).
pub(super) fn mastery_numeral(stage: usize) -> &'static str {
    match stage {
        0 | 1 => "I",
        2 => "II",
        _ => "III",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_run_metrics_have_staged_thresholds_and_rewards() {
        assert_eq!(SCORE_RUNE_THRESHOLDS, [300, 900, 1_800]);
        assert_eq!(DATA_CHARGE_THRESHOLDS, [3, 6, 9]);
        assert_eq!(BUG_BREACH_THRESHOLDS, [1, 3, 5]);

        assert_eq!(streak_multiplier(0), 1);
        assert_eq!(streak_multiplier(2), 1);
        assert_eq!(streak_multiplier(3), 2);
        assert_eq!(streak_multiplier(4), 2);
        assert_eq!(streak_multiplier(5), 2);
        assert_eq!(streak_multiplier(6), 3);
        assert_eq!(streak_multiplier(7), 3);
        assert_eq!(score_award_for_streak(2), 100);
        assert_eq!(score_award_for_streak(3), 200);
        assert_eq!(score_award_for_streak(6), 300);

        assert_eq!(InsightStage::from_score(299), InsightStage::Unlit);
        assert_eq!(InsightStage::from_score(300), InsightStage::RuneOne);
        assert_eq!(InsightStage::from_score(301), InsightStage::RuneOne);
        assert_eq!(InsightStage::from_score(899), InsightStage::RuneOne);
        assert_eq!(InsightStage::from_score(900), InsightStage::RuneTwo);
        assert_eq!(InsightStage::from_score(901), InsightStage::RuneTwo);
        assert_eq!(InsightStage::from_score(1_799), InsightStage::RuneTwo);
        assert_eq!(InsightStage::from_score(1_800), InsightStage::RuneThree);
        assert_eq!(InsightStage::from_score(1_801), InsightStage::RuneThree);
        assert_eq!(threshold_stage(2, &DATA_CHARGE_THRESHOLDS), 0);
        assert_eq!(threshold_stage(3, &DATA_CHARGE_THRESHOLDS), 1);
        assert_eq!(threshold_stage(4, &DATA_CHARGE_THRESHOLDS), 1);
        assert_eq!(threshold_stage(5, &DATA_CHARGE_THRESHOLDS), 1);
        assert_eq!(threshold_stage(6, &DATA_CHARGE_THRESHOLDS), 2);
        assert_eq!(threshold_stage(7, &DATA_CHARGE_THRESHOLDS), 2);
        assert_eq!(threshold_stage(8, &DATA_CHARGE_THRESHOLDS), 2);
        assert_eq!(threshold_stage(9, &DATA_CHARGE_THRESHOLDS), 3);
        assert_eq!(threshold_stage(10, &DATA_CHARGE_THRESHOLDS), 3);
        assert_eq!(threshold_stage(0, &BUG_BREACH_THRESHOLDS), 0);
        assert_eq!(threshold_stage(1, &BUG_BREACH_THRESHOLDS), 1);
        assert_eq!(threshold_stage(2, &BUG_BREACH_THRESHOLDS), 1);
        assert_eq!(threshold_stage(3, &BUG_BREACH_THRESHOLDS), 2);
        assert_eq!(threshold_stage(4, &BUG_BREACH_THRESHOLDS), 2);
        assert_eq!(threshold_stage(5, &BUG_BREACH_THRESHOLDS), 3);
        assert_eq!(threshold_stage(6, &BUG_BREACH_THRESHOLDS), 3);
    }
}
