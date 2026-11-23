use canonwright_story::prelude::*;

const FIXTURE: &str = include_str!("fixtures/the-salt-road.json");

fn cid(value: &str) -> CharacterId {
    CharacterId::new(value).unwrap()
}

fn sid(value: &str) -> SceneId {
    SceneId::new(value).unwrap()
}

fn fid(value: &str) -> FactId {
    FactId::new(value).unwrap()
}

fn story() -> Story {
    Story::from_json(FIXTURE).expect("the shipped fixture must parse")
}

#[test]
fn the_shipped_fixture_is_structurally_sound() {
    let story = story();
    let errors = story.check();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn the_fixture_survives_a_round_trip_without_losing_its_timeline() {
    let story = story();
    let encoded = story.to_json().unwrap();
    let decoded = Story::from_json(&encoded).unwrap();
    assert_eq!(story, decoded);
    assert_eq!(decoded.state_at(&sid("s3")), story.state_at(&sid("s3")));
}

#[test]
fn the_fixture_demonstrates_temporal_isolation() {
    let story = story();

    let before = story.state_before(&sid("s3"));
    assert!(
        !before.knows(&cid("mara"), &fid("harek-burned-the-ford")),
        "she learns it during scene three, not before it"
    );
    let after = story.state_at(&sid("s3"));
    assert!(after.knows(&cid("mara"), &fid("harek-burned-the-ford")));

    assert!(
        !story
            .state_before(&sid("s2"))
            .known_to_reader
            .contains(&fid("harek-burned-the-ford")),
        "the reader has not been told yet either"
    );
}

#[test]
fn the_fixture_records_relationships_as_edges() {
    let story = story();
    let mara = story.characters.get(&cid("mara")).unwrap();
    let harek = mara
        .relationships
        .iter()
        .find(|edge| edge.to == TargetRef::character(cid("harek")))
        .expect("mara has an opinion about harek");
    assert_eq!(harek.value.trust, 2);
    assert!(harek.value.note.contains("held the ford"));
}

#[test]
fn canon_scenes_that_were_never_reviewed_are_reported_as_dirty() {
    let story = story();
    assert_eq!(story.dirty_scenes(), vec![sid("s1"), sid("s2")]);

    let mut story = story;
    for id in ["s1", "s2"] {
        story.mark_reviewed(&sid(id)).unwrap();
    }
    assert!(story.dirty_scenes().is_empty());
}

#[test]
fn the_fixture_pays_off_one_thread_and_owes_another() {
    let story = story();

    let state = story.state_at(&sid("s3"));
    assert_eq!(
        state
            .thread(&ThreadId::new("who-burned-the-ford").unwrap())
            .status,
        ThreadStatus::PaidOff
    );
    assert_eq!(
        state
            .thread(&ThreadId::new("the-forged-seal").unwrap())
            .status,
        ThreadStatus::Planted
    );
}

#[test]
fn the_fixture_lints_without_inventing_errors() {
    let story = story();
    let tickets = story.lint();

    // Scene four is planned, so it contributes nothing and is not linted.
    assert!(
        !tickets.iter().any(|ticket| ticket.scene == sid("s4")),
        "{tickets:?}"
    );
    // Every finding is advisory and every claim it makes is true of the timeline.
    for ticket in &tickets {
        assert!(!ticket.blocking);
        assert!(
            !matches!(story.substantiate(ticket), Substantiation::Refuted(_)),
            "{ticket:?} was refuted by the timeline"
        );
    }
}

#[test]
fn the_forged_seal_is_still_owed_at_the_end_of_the_drafted_book() {
    let story = story();
    let thread = story
        .threads
        .get(&ThreadId::new("the-forged-seal").unwrap())
        .unwrap();
    assert_eq!(thread.promised_payoff, Some(sid("s4")));
    assert_eq!(thread.opened_in, sid("s2"));
}
