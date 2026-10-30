use expert_tui::pet::{Art, Pet, load_pack};
use std::time::{Duration, Instant};

const MINIMAL: &str = r##"{
 "palette": {"a": "#112233"},
 "frames": {"sit": ["a.", ".a"], "wave": ["aa", "aa"]},
 "idle": {"frames": [["sit", 0]]},
 "clicks": ["hello"],
 "actions": [{"id": "hello", "frames": [["wave", 500]], "quotes": [[0, "hi"]]}]
}"##;

#[test]
fn custom_pack_plays_its_own_idle_and_click_action() {
    let art = Art::parse(MINIMAL).unwrap();
    assert_eq!(art.dimensions(), (2, 2));
    let mut pet = Pet::default();
    pet.art = art;
    pet.visible = true;
    let now = Instant::now();
    assert_eq!(pet.frame(now), "sit");
    pet.click(now, false);
    assert_eq!(pet.frame(now), "wave");
    assert_eq!(pet.quote(now), "hi");
    pet.tick(now + Duration::from_millis(500), false);
    assert_eq!(pet.action, 0);
    assert_eq!(pet.frame(now + Duration::from_millis(500)), "sit");
    // The line lingers for the pack's quote time after the action ends.
    assert_eq!(pet.quote(now + Duration::from_millis(600)), "hi");
    assert_eq!(pet.quote(now + Duration::from_millis(2600)), "");
}

#[test]
fn packs_load_by_path_and_bundled_default_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cat.json");
    std::fs::write(&path, MINIMAL).unwrap();
    assert_eq!(
        load_pack(path.to_str().unwrap()).unwrap().dimensions(),
        (2, 2)
    );
    assert_eq!(load_pack("megumin").unwrap().dimensions(), (64, 80));
    assert!(load_pack("../escape").is_err());
}

#[test]
fn invalid_packs_are_rejected_with_a_reason() {
    for (broken, reason) in [
        (MINIMAL.replace("\"aa\", \"aa\"", "\"aaa\", \"aa\""), "尺寸"),
        (
            MINIMAL.replace("\"a.\", \".a\"", "\"z.\", \".a\""),
            "调色板",
        ),
        (
            MINIMAL.replace("[[\"sit\", 0]]", "[[\"missing\", 0]]"),
            "不存在的帧",
        ),
        (MINIMAL.replace("[\"hello\"]", "[\"nope\"]"), "不存在的动作"),
        (MINIMAL.replace("#112233", "red"), "#RRGGBB"),
        (MINIMAL.replace("500", "0"), "时长"),
    ] {
        let error = Art::parse(&broken).unwrap_err();
        assert!(error.contains(reason), "{error}");
    }
}
