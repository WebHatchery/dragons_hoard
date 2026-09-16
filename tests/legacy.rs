#![allow(unused_imports)]

//! Regression coverage migrated from source modules into the crate test target.
//!
//! The wrappers mirror the production module that originally owned each test.
//! This keeps `use super::*` useful while making the test files integration
//! tests against the library's public seams.

mod test_prelude {
    pub use dragons_hoard::audio::Sfx;
    pub use dragons_hoard::data::{
        BonusConfig, Evaluation, FeatureBuyConfig, GambleConfig, GameConfig, GameData,
        HoldSpinConfig, Jackpots, MachineDef, RiteDef, RiteKind, SeamConfig, SymbolDef, MACHINES,
        MAX_RUN,
    };
    pub use dragons_hoard::engine;
    pub use dragons_hoard::engine::evaluate::{Win, WinSource};
    pub use dragons_hoard::engine::sim::{run, SimConfig};
    pub use dragons_hoard::engine::{
        CascadeStep, EvalContext, Grid, SpinMode, SpinOutcome, SpinResult,
    };
    pub use dragons_hoard::game::screens::Screen;
    pub use dragons_hoard::game::Game;
    pub use dragons_hoard::music;
    pub use dragons_hoard::music::{Arrangement, Mood, Track};
    pub use dragons_hoard::state::achievements::{
        AchievementBook, AchievementDef, AchievementProgress, ConditionKind, FeatureRound,
        UnlockCondition,
    };
    pub use dragons_hoard::state::bonus::{BonusCell, BonusOutcome, BonusRound};
    pub use dragons_hoard::state::celebration::{Celebration, CelebrationKind, CelebrationQueue};
    pub use dragons_hoard::state::gamble::{GambleBlocked, GambleFlip, GambleRound, Scale};
    pub use dragons_hoard::state::hints::{Counter, HintBook, HintDef, HintProgress};
    pub use dragons_hoard::state::history::{Cause, History};
    pub use dragons_hoard::state::holdspin::{HoldSpinOutcome, HoldSpinRound};
    pub use dragons_hoard::state::jackpot::{JackpotState, JackpotWin};
    pub use dragons_hoard::state::ledger::{Ledger, OpenRound};
    pub use dragons_hoard::state::limits::{Breach, Cap, LimitChoices, LimitState, SessionClock};
    pub use dragons_hoard::state::preferences::Preferences;
    pub use dragons_hoard::state::proof::{Commitment, RecordedMode, Verdict};
    pub use dragons_hoard::state::ruin::Lifeline;
    pub use dragons_hoard::state::rules::{Rule, Topic};
    pub use dragons_hoard::state::seam::{SeamOutcome, SeamRound};
    pub use dragons_hoard::state::sessions::{EndedBy, Session, SessionLog};
    pub use dragons_hoard::state::spin::SpinPhase;
    pub use dragons_hoard::state::wallet::Wallet;
    pub use dragons_hoard::state::{FreeSpinState, GameSession, SpinBlocked, SpinResolution};
    pub use dragons_hoard::ui::nav::Hit;
    pub use dragons_hoard::ui::{frame, naming};
    pub use dragons_hoard::ui::{UiAction, UiContext};
    pub use macroquad::prelude::{vec2, Color, KeyCode, MouseButton, Rect, Vec2};
    pub use macroquad_toolkit::achievements::{Achievement, Achievements};
    pub use macroquad_toolkit::rng::SeededRng;
    pub use macroquad_toolkit::synth::audit::{measure, Measured};
    pub use macroquad_toolkit::synth::{render_wav, SynthConfig, Voice, Wave};
    pub use macroquad_toolkit::ui::{Pointer, VirtualUi};
    pub use serde_json;
}

mod actions {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::actions::*;
    #[path = "../legacy/src/actions/tests.rs"]
    mod tests;
}

mod audio {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::audio::*;
    pub mod audit {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::audio::audit::*;
        #[path = "../../legacy/src/audio/audit/tests.rs"]
        mod tests;
    }
    #[path = "../legacy/src/audio/tests.rs"]
    mod tests;
}

mod data {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::data::*;
    pub mod load {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::data::load::*;
        #[path = "../../legacy/src/data/load/tests.rs"]
        mod tests;
    }
    #[path = "../legacy/src/data/ante_tests.rs"]
    mod ante_tests;
    #[path = "../legacy/src/data/tests.rs"]
    mod tests;
}

mod engine {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::engine::*;
    pub mod cascade {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::engine::cascade::*;
        #[path = "../../legacy/src/engine/cascade/tests.rs"]
        mod tests;
    }
    pub mod cluster {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::engine::cluster::*;
        #[path = "../../legacy/src/engine/cluster/tests.rs"]
        mod tests;
    }
    pub mod evaluate {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::engine::evaluate::*;
        pub mod ways {
            pub use crate::test_prelude::*;
            pub use dragons_hoard::engine::evaluate::ways::*;
            #[path = "../../../legacy/src/engine/evaluate/ways/tests.rs"]
            mod tests;
        }
        #[path = "../../legacy/src/engine/evaluate/tests.rs"]
        mod tests;
    }
    pub mod reels {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::engine::reels::*;
        #[path = "../../legacy/src/engine/reels/tests.rs"]
        mod tests;
    }
    pub mod seam {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::engine::seam::*;
        #[path = "../../legacy/src/engine/seam/tests.rs"]
        mod tests;
    }
    pub mod sim {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::engine::sim::*;
        #[path = "../../legacy/src/engine/sim/tests.rs"]
        mod tests;
    }
    pub mod soak {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::engine::soak::*;
        #[path = "../../legacy/src/engine/soak/tests.rs"]
        mod tests;
    }
    #[path = "../legacy/src/engine/tests.rs"]
    mod tests;
}

mod game {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::game::*;
    pub mod motion {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::game::motion::*;
        #[path = "../../legacy/src/game/motion/tests.rs"]
        mod tests;
    }
    #[path = "../legacy/src/game/screens/tests.rs"]
    mod screens_tests;
}

mod music {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::music::*;
    #[path = "../legacy/src/music/tests.rs"]
    mod tests;
}

mod state {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::state::*;
    pub mod achievements {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::achievements::*;
        #[path = "../../legacy/src/state/achievements/tests.rs"]
        mod tests;
    }
    pub mod autospin {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::autospin::*;
        #[path = "../../legacy/src/state/autospin/tests.rs"]
        mod tests;
    }
    pub mod bonus {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::bonus::*;
        #[path = "../../legacy/src/state/bonus/tests.rs"]
        mod tests;
    }
    pub mod celebration {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::celebration::*;
        #[path = "../../legacy/src/state/celebration/tests.rs"]
        mod tests;
    }
    pub mod compat {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::compat::*;
        #[path = "../../legacy/src/state/compat/persistence_of_play.rs"]
        mod persistence_of_play;
        #[path = "../../legacy/src/state/compat/tests.rs"]
        mod tests;
    }
    pub mod featurebuy {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::featurebuy::*;
        #[path = "../../legacy/src/state/featurebuy/tests.rs"]
        mod tests;
    }
    pub mod floor {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::floor::*;
        #[path = "../../legacy/src/state/floor/tests.rs"]
        mod tests;
    }
    pub mod gamble {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::gamble::*;
        #[path = "../../legacy/src/state/gamble/tests.rs"]
        mod tests;
    }
    pub mod hints {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::hints::*;
        #[path = "../../legacy/src/state/hints/reachability.rs"]
        mod reachability;
        #[path = "../../legacy/src/state/hints/tests.rs"]
        mod tests;
    }
    pub mod history {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::history::*;
        #[path = "../../legacy/src/state/history/tests.rs"]
        mod tests;
    }
    pub mod hoard {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::hoard::*;
        #[path = "../../legacy/src/state/hoard/tests.rs"]
        mod tests;
    }
    pub mod holdspin {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::holdspin::*;
        #[path = "../../legacy/src/state/holdspin/tests.rs"]
        mod tests;
    }
    pub mod jackpot {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::jackpot::*;
        #[path = "../../legacy/src/state/jackpot/tests.rs"]
        mod tests;
    }
    pub mod ledger {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::ledger::*;
        #[path = "../../legacy/src/state/ledger/tests.rs"]
        mod tests;
    }
    pub mod limits {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::limits::*;
        #[path = "../../legacy/src/state/limits/tests.rs"]
        mod tests;
    }
    pub mod persist {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::persist::*;
        #[path = "../../legacy/src/state/persist/tests.rs"]
        mod tests;
    }
    pub mod preferences {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::preferences::*;
        #[path = "../../legacy/src/state/preferences/tests.rs"]
        mod tests;
        #[path = "../../legacy/src/state/preferences/text_scale_tests.rs"]
        mod text_scale_tests;
    }
    pub mod profile {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::profile::*;
        pub mod features {
            pub use crate::test_prelude::*;
            pub use dragons_hoard::state::profile::features::*;
            #[path = "../../../legacy/src/state/profile/features/shape_tests.rs"]
            mod shape_tests;
            #[path = "../../../legacy/src/state/profile/features/tier_tests.rs"]
            mod tier_tests;
        }
        #[path = "../../legacy/src/state/profile/tests.rs"]
        mod tests;
    }
    pub mod proof {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::proof::*;
        #[path = "../../legacy/src/state/proof/tests.rs"]
        mod tests;
    }
    pub mod ruin {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::ruin::*;
        #[path = "../../legacy/src/state/ruin/tests.rs"]
        mod tests;
    }
    pub mod rules {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::rules::*;
        #[path = "../../legacy/src/state/rules/coverage.rs"]
        mod coverage;
        #[path = "../../legacy/src/state/rules/tests.rs"]
        mod tests;
    }
    pub mod save {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::save::*;
        #[path = "../../legacy/src/state/save/tests.rs"]
        mod tests;
    }
    pub mod seam {
        pub use crate::engine::seam;
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::seam::*;
        #[path = "../../legacy/src/state/seam/tests.rs"]
        mod tests;
    }
    pub mod sessions {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::sessions::*;
        #[path = "../../legacy/src/state/sessions/tests.rs"]
        mod tests;
    }
    pub mod spin {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::spin::*;
        #[path = "../../legacy/src/state/spin/tests.rs"]
        mod tests;
    }
    pub mod wallet {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::wallet::*;
        #[path = "../../legacy/src/state/wallet/tests.rs"]
        mod tests;
    }
    #[path = "../legacy/src/state/tests.rs"]
    mod tests;
}

mod ui {
    pub use crate::test_prelude::*;
    pub use dragons_hoard::ui::*;
    pub mod achievements {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::achievements::*;
        #[path = "../../legacy/src/ui/achievements/tests.rs"]
        mod tests;
    }
    pub mod frame {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::frame::*;
        #[path = "../../legacy/src/ui/frame/tests.rs"]
        mod tests;
    }
    pub mod hint {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::hint::*;
        #[path = "../../legacy/src/ui/hint/tests.rs"]
        mod tests;
    }
    pub mod history {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::history::*;
        #[path = "../../legacy/src/ui/history/tests.rs"]
        mod tests;
    }
    pub mod legibility {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::legibility::*;
        #[path = "../../legacy/src/ui/legibility/tests.rs"]
        mod tests;
    }
    pub mod limits {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::limits::*;
        #[path = "../../legacy/src/ui/limits/tests.rs"]
        mod tests;
    }
    pub mod lines {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::lines::*;
        #[path = "../../legacy/src/ui/lines/tests.rs"]
        mod tests;
    }
    pub mod machines {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::machines::*;
        #[path = "../../legacy/src/ui/machines/tests.rs"]
        mod tests;
    }
    pub mod menu {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::menu::*;
        #[path = "../../legacy/src/ui/menu/tests.rs"]
        mod tests;
    }
    pub mod naming {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::naming::*;
        #[path = "../../legacy/src/ui/naming/tests.rs"]
        mod tests;
    }
    pub mod nav {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::nav::*;
        #[path = "../../legacy/src/ui/nav/tests.rs"]
        mod tests;
    }
    pub mod paylines {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::paylines::*;
        #[path = "../../legacy/src/ui/paylines/tests.rs"]
        mod tests;
    }
    pub mod paytable {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::paytable::*;
        #[path = "../../legacy/src/ui/paytable/tests.rs"]
        mod tests;
    }
    pub mod proof {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::proof::*;
        #[path = "../../legacy/src/ui/proof/tests.rs"]
        mod tests;
    }
    pub mod reality {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::reality::*;
        #[path = "../../legacy/src/ui/reality/tests.rs"]
        mod tests;
    }
    pub mod reels {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::reels::*;
        #[path = "../../legacy/src/ui/reels/chrome.rs"]
        mod chrome;
        #[path = "../../legacy/src/ui/reels/tests.rs"]
        mod tests;
    }
    pub mod rules {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::state::rules;
        pub use dragons_hoard::ui::rules::*;
        #[path = "../../legacy/src/ui/rules/tests.rs"]
        mod tests;
    }
    pub mod seam {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::seam::*;
        #[path = "../../legacy/src/ui/seam/tests.rs"]
        mod tests;
    }
    pub mod sessionover {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::sessionover::*;
        #[path = "../../legacy/src/ui/sessionover/tests.rs"]
        mod tests;
    }
    pub mod sessions {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::sessions::*;
        #[path = "../../legacy/src/ui/sessions/tests.rs"]
        mod tests;
    }
    pub mod shortcuts {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::shortcuts::*;
        #[path = "../../legacy/src/ui/shortcuts/tests.rs"]
        mod tests;
    }
    pub mod symbols {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::symbols::*;
        pub mod legible {
            pub use crate::test_prelude::*;
            pub use dragons_hoard::ui::symbols::legible::*;
            #[path = "../../../legacy/src/ui/symbols/legible/tests.rs"]
            mod tests;
        }
        #[path = "../../legacy/src/ui/symbols/tests.rs"]
        mod tests;
    }
    pub mod theme {
        pub use crate::test_prelude::*;
        pub use dragons_hoard::ui::theme::*;
        #[path = "../../legacy/src/ui/theme/tests.rs"]
        mod tests;
    }
}
