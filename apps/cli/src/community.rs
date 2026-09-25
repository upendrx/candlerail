//! Strategies shared by the community (the files in `community/strategies/`),
//! compiled into the binary so the app can show them offline.

pub const ALL: &[(&str, &str)] = &[
    ("btc-4h-engulfing-3r", include_str!("../../../community/strategies/btc-4h-engulfing-3r.share.json")),
    ("eth-daily-inside-bar", include_str!("../../../community/strategies/eth-daily-inside-bar.share.json")),
    ("btc-daily-donchian-guarded", include_str!("../../../community/strategies/btc-daily-donchian-guarded.share.json")),
];

#[cfg(test)]
mod tests {
    use candlerail_core::Strategy;

    #[test]
    fn community_files_load_as_strategies() {
        for (id, text) in super::ALL {
            let s = Strategy::from_ai_text(text).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(s.validate().is_empty(), "{id}: {:?}", s.validate());
            let v: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(v["result"]["trades"].is_number(), "{id} needs a recorded result");
        }
    }
}
