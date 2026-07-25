//! The machine catalog: which cabinets exist and which files each is made of.
//!
//! Deliberately the only Rust a new machine needs. Everything that makes a
//! cabinet what it is — symbols, strips, paytable, feature rules, evaluation
//! model (§5.14), cascades (§5.15) — is JSON; this file just says which files to
//! embed, because `include_str!` runs at compile time.

/// One playable machine: a complete, self-contained set of maths and content.
///
/// Everything that makes a machine what it is — symbols, strips, paytable,
/// feature rules, jackpot tiers — is JSON. The only Rust here is the list of
/// which files to embed, because `include_str!` runs at compile time.
///
/// Adding a machine is: drop a directory under `assets/data/machines/`, add an
/// entry to [`MACHINES`], and re-run the RTP sim — which tests **every** machine
/// (§4), so a new one cannot ship out of band.
#[derive(Debug)]
pub struct MachineDef {
    pub id: &'static str,
    /// One line for the picker, describing how this machine plays.
    pub blurb: &'static str,
    pub(super) config: &'static str,
    pub(super) paytable: &'static str,
    pub(super) reels: &'static str,
    pub(super) paylines: &'static str,
    pub(super) freespins: &'static str,
    pub(super) jackpots: &'static str,
    pub(super) holdspin: &'static str,
    pub(super) cascade: Option<&'static str>,
    pub(super) featurebuy: &'static str,
}

macro_rules! machine {
    ($id:literal, $dir:literal, $blurb:literal) => {
        MachineDef {
            id: $id,
            blurb: $blurb,
            config: include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/game_config.json"
            )),
            paytable: include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/paytable.json"
            )),
            reels: include_str!(concat!("../../assets/data/machines/", $dir, "/reels.json")),
            paylines: include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/paylines.json"
            )),
            freespins: include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/freespins.json"
            )),
            jackpots: include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/jackpots.json"
            )),
            holdspin: include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/holdspin.json"
            )),
            featurebuy: include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/featurebuy.json"
            )),
            cascade: None,
        }
    };
}

/// A cabinet whose reels cascade (§5.15). Same fields as `machine!`, plus the
/// cascade config — a separate macro so the three that do not cascade carry no
/// mention of it.
macro_rules! cascading_machine {
    ($id:literal, $dir:literal, $blurb:literal) => {
        MachineDef {
            cascade: Some(include_str!(concat!(
                "../../assets/data/machines/",
                $dir,
                "/cascade.json"
            ))),
            ..machine!($id, $dir, $blurb)
        }
    };
}

/// The symbol sets, by name.
///
/// A set is what the symbols *are* — names, art and colour. Several cabinets
/// share one, and a cabinet's own `paytable.json` says what each pays there
/// (§5.41). Held as a lookup rather than a field on the machine so a set can be
/// added without touching every cabinet that does not use it.
pub fn symbol_set(name: &str) -> Option<&'static str> {
    Some(match name {
        "hoard" => include_str!("../../assets/data/symbols/hoard.json"),
        "frost" => include_str!("../../assets/data/symbols/frost.json"),
        "tidepool" => include_str!("../../assets/data/symbols/tidepool.json"),
        "ember" => include_str!("../../assets/data/symbols/ember.json"),
        "spire" => include_str!("../../assets/data/symbols/spire.json"),
        "slide" => include_str!("../../assets/data/symbols/slide.json"),
        _ => return None,
    })
}

pub static MACHINES: &[MachineDef] = &[
    machine!(
        "dragon",
        "dragon",
        "Medium volatility. Frequent coin and gem wins, doubled free spins."
    ),
    machine!(
        "frost",
        "frost",
        "High volatility. Rarer wins, far bigger, with tripled free spins."
    ),
    machine!(
        "ways",
        "ways",
        "243 ways to win — no paylines. Symbols pay from the left wherever they land."
    ),
    machine!(
        "wyrmspire",
        "wyrmspire",
        "Shifting reels. Every reel is a different height each spin, so the ways change with it."
    ),
    cascading_machine!(
        "avalanche",
        "avalanche",
        "Cascading 243 ways. Winners are cleared, the grid refills, and the multiplier climbs."
    ),
    cascading_machine!(
        "tidepool",
        "tidepool",
        "Cluster pays on a 6x5 grid. Groups that touch pay together, wherever they are."
    ),
];

/// Look a machine up by id, falling back to the first so a stale saved id can
/// never leave the player with no machine at all.
pub fn machine_by_id(id: &str) -> &'static MachineDef {
    MACHINES
        .iter()
        .find(|machine| machine.id == id)
        .unwrap_or(&MACHINES[0])
}
