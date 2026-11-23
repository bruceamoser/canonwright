use canonwright_story::prelude::*;

fn cid(value: &str) -> CharacterId {
    CharacterId::new(value).unwrap()
}

fn sid(value: &str) -> SceneId {
    SceneId::new(value).unwrap()
}

fn rid(value: &str) -> FactId {
    FactId::new(value).unwrap()
}

fn scene(id: &str, index: u32, status: SceneStatus) -> Scene {
    Scene {
        id: sid(id),
        index,
        title: format!("Scene {index}"),
        pov: Some(cid("mara")),
        present: vec![cid("mara")],
        status,
        value_start: "safe".to_owned(),
        value_end: "compromised".to_owned(),
        ..Scene::default()
    }
}

fn depends_on(mut scene: Scene, dependency: &str) -> Scene {
    scene.dependencies.push(CausalLink {
        kind: CausalKind::Because,
        scene: sid(dependency),
        note: String::new(),
    });
    scene
}

fn fixture() -> Story {
    let mut story = Story::default();
    story.meta.title = "The Salt Road".to_owned();
    story.characters.insert(
        cid("mara"),
        Character {
            name: "Mara Vale".to_owned(),
            ..Character::default()
        },
    );
    story.facts.insert(
        rid("truce-broken"),
        Fact {
            statement: "The truce was broken at the ford.".to_owned(),
            secret: false,
        },
    );

    let mut one = scene("s1", 1, SceneStatus::Canon);
    one.delta = SceneDelta::new(vec![SceneChange::at(
        5,
        Change::Learned {
            character: cid("mara"),
            fact: rid("truce-broken"),
        },
    )]);
    let two = depends_on(scene("s2", 2, SceneStatus::Canon), "s1");
    let three = depends_on(scene("s3", 3, SceneStatus::Drafted), "s2");

    story.scenes = vec![one, two, three];
    story
}

fn reviewed() -> Story {
    let mut story = fixture();
    for id in ["s1", "s2"] {
        story.mark_reviewed(&sid(id)).unwrap();
    }
    story
}

#[test]
fn a_written_scene_is_accepted_and_the_story_changes() {
    let mut story = reviewed();
    let mut four = depends_on(scene("s4", 4, SceneStatus::Drafted), "s3");
    four.title = "The ford".to_owned();

    let outcome = story.write_scene(four);
    assert!(outcome.accepted, "{:?}", outcome.errors);
    assert!(outcome.created);
    assert_eq!(story.scenes.len(), 4);
    assert_eq!(story.scene(&sid("s4")).unwrap().title, "The ford");
}

#[test]
fn rewriting_a_scene_does_not_create_a_duplicate() {
    let mut story = reviewed();
    let mut two = story.scene(&sid("s2")).unwrap().clone();
    two.title = "The ford, again".to_owned();

    let outcome = story.write_scene(two);
    assert!(outcome.accepted);
    assert!(!outcome.created);
    assert_eq!(story.scenes.len(), 3);
    assert_eq!(story.scene(&sid("s2")).unwrap().title, "The ford, again");
}

#[test]
fn an_invalid_scene_is_rejected_and_leaves_the_story_untouched() {
    let mut story = reviewed();
    let before = story.clone();
    let mut broken = scene("s9", 9, SceneStatus::Drafted);
    broken.pov = Some(cid("nobody"));

    let outcome = story.write_scene(broken);
    assert!(!outcome.accepted);
    assert!(!outcome.errors.is_empty());
    assert_eq!(story, before, "a rejected write must not half-apply");
}

#[test]
fn a_new_canon_scene_is_dirty_until_it_is_reviewed() {
    let mut story = fixture();
    let outcome = story.set_scene_status(&sid("s3"), SceneStatus::Canon);
    assert!(outcome.accepted, "{:?}", outcome.errors);

    assert!(story.is_dirty(&sid("s3")));
    story.mark_reviewed(&sid("s3")).unwrap();
    assert!(!story.is_dirty(&sid("s3")));
}

#[test]
fn a_drafted_scene_is_never_dirty() {
    let story = fixture();
    assert!(!story.is_dirty(&sid("s3")), "unfinished is not stale");
}

#[test]
fn changing_an_upstream_delta_makes_a_reviewed_scene_dirty_again() {
    let mut story = reviewed();
    assert!(!story.is_dirty(&sid("s2")));

    let mut one = story.scene(&sid("s1")).unwrap().clone();
    one.delta.changes.push(SceneChange::at(
        9,
        Change::Learned {
            character: cid("mara"),
            fact: rid("truce-broken"),
        },
    ));
    let outcome = story.write_scene(one);
    assert!(outcome.accepted, "{:?}", outcome.errors);

    assert!(
        outcome.dirty.contains(&sid("s2")),
        "s2 was reviewed against the old s1: {:?}",
        outcome.dirty
    );
    assert!(story.is_dirty(&sid("s2")));
    assert!(
        !story.is_dirty(&sid("s1")),
        "a scene is not invalidated by its own edit"
    );
}

#[test]
fn rewriting_prose_without_changing_the_delta_leaves_dependents_clean() {
    let mut story = reviewed();
    let mut one = story.scene(&sid("s1")).unwrap().clone();
    one.summary = "A quiet morning that ends badly.".to_owned();
    one.title = "Morning".to_owned();

    let outcome = story.write_scene(one);
    assert!(outcome.accepted, "{:?}", outcome.errors);
    assert!(
        outcome.dirty.is_empty(),
        "words changed, state did not: {:?}",
        outcome.dirty
    );
    assert!(!story.is_dirty(&sid("s2")));
}

#[test]
fn re_reviewing_clears_the_dirty_list() {
    let mut story = reviewed();
    let mut one = story.scene(&sid("s1")).unwrap().clone();
    one.delta.changes.clear();
    assert!(story.write_scene(one).accepted);
    assert_eq!(story.dirty_scenes(), vec![sid("s2")]);

    story.mark_reviewed(&sid("s2")).unwrap();
    assert!(story.dirty_scenes().is_empty());
}

#[test]
fn setting_a_status_on_an_unknown_scene_reports_instead_of_panicking() {
    let mut story = fixture();
    let outcome = story.set_scene_status(&sid("nope"), SceneStatus::Canon);
    assert!(!outcome.accepted);
    assert_eq!(outcome.errors.len(), 1);
}

#[test]
fn the_premises_digest_covers_transitive_dependencies() {
    let story = fixture();
    let digest = story.premises_digest(&sid("s3")).unwrap();
    assert_eq!(digest.len(), 64, "blake3 hex digest");

    // s3 depends on s2 depends on s1, so both leave a mark.
    let mut shallow = story.clone();
    shallow
        .scene_mut(&sid("s3"))
        .unwrap()
        .dependencies
        .retain(|link| link.scene != sid("s2"));
    assert_ne!(shallow.premises_digest(&sid("s3")).unwrap(), digest);
}
