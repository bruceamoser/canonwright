use canonwright_story::prelude::*;

fn cid(value: &str) -> CharacterId {
    CharacterId::new(value).unwrap()
}

fn sid(value: &str) -> SceneId {
    SceneId::new(value).unwrap()
}

fn fid(value: &str) -> FactId {
    FactId::new(value).unwrap()
}

fn tid(value: &str) -> ThreadId {
    ThreadId::new(value).unwrap()
}

fn rid(value: &str) -> RuleId {
    RuleId::new(value).unwrap()
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

/// Three scenes: Mara learns the truce broke in scene one and learns who broke
/// it in scene two.
fn fixture() -> Story {
    let mut story = Story::default();
    story.meta.title = "The Salt Road".to_owned();
    story.characters.insert(
        cid("mara"),
        Character {
            name: "Mara Vale".to_owned(),
            want: "Get the salt caravans moving again".to_owned(),
            need: "Admit she was wrong to leave".to_owned(),
            ..Character::default()
        },
    );
    story.characters.insert(
        cid("harek"),
        Character {
            name: "Harek".to_owned(),
            ..Character::default()
        },
    );
    story.facts.insert(
        fid("truce-broken"),
        Fact {
            statement: "The truce was broken at the ford.".to_owned(),
            secret: false,
        },
    );
    story.facts.insert(
        fid("harek-broke-it"),
        Fact {
            statement: "Harek ordered the ford burned.".to_owned(),
            secret: true,
        },
    );

    let mut one = scene("s1", 1, SceneStatus::Canon);
    one.delta = SceneDelta::new(vec![
        SceneChange::at(
            5,
            Change::Learned {
                character: cid("mara"),
                fact: fid("truce-broken"),
            },
        ),
        SceneChange::unplaced(Change::Revealed {
            fact: fid("truce-broken"),
        }),
    ]);

    let mut two = depends_on(scene("s2", 2, SceneStatus::Canon), "s1");
    two.delta = SceneDelta::new(vec![
        SceneChange::at(
            12,
            Change::Learned {
                character: cid("mara"),
                fact: fid("harek-broke-it"),
            },
        ),
        SceneChange::unplaced(Change::Revealed {
            fact: fid("harek-broke-it"),
        }),
    ]);

    let mut three = depends_on(scene("s3", 3, SceneStatus::Drafted), "s2");
    three.delta = SceneDelta::default();

    let mut four = depends_on(scene("s4", 4, SceneStatus::Drafted), "s3");
    four.delta = SceneDelta::default();

    let mut five = depends_on(scene("s5", 5, SceneStatus::Planned), "s1");
    five.delta = SceneDelta::default();

    story.scenes = vec![one, two, three, four, five];
    story
}

#[test]
fn state_before_a_scene_cannot_see_that_scene() {
    let story = fixture();

    let before_one = story.state_before(&sid("s1"));
    assert!(!before_one.knows(&cid("mara"), &fid("truce-broken")));

    let before_two = story.state_before(&sid("s2"));
    assert!(before_two.knows(&cid("mara"), &fid("truce-broken")));
    assert!(
        !before_two.knows(&cid("mara"), &fid("harek-broke-it")),
        "scene two is where she learns it; the state before it must not contain it"
    );

    let after_two = story.state_at(&sid("s2"));
    assert!(after_two.knows(&cid("mara"), &fid("harek-broke-it")));
}

#[test]
fn planned_scenes_contribute_no_state() {
    let mut story = fixture();
    let planned = story.scenes.iter_mut().find(|s| s.id == sid("s5")).unwrap();
    planned.delta = SceneDelta::new(vec![SceneChange::unplaced(Change::Death {
        character: cid("mara"),
    })]);

    let last = story.state_at(&sid("s5"));
    assert!(
        last.character(&cid("mara")).alive,
        "a beat on the board cannot kill a character"
    );
}

#[test]
fn a_knowledge_leak_is_settled_by_the_timeline_not_by_opinion() {
    let story = fixture();

    // She learns it on line 12; a reviewer citing line 9 is right.
    let leak = Ticket::new(
        TicketCode::KnowledgeLeak,
        sid("s2"),
        "Mara names Harek as the arsonist before she could know",
    )
    .at(9)
    .claiming(Claim::Knowledge {
        character: cid("mara"),
        fact: fid("harek-broke-it"),
    });
    assert_eq!(story.substantiate(&leak), Substantiation::Confirmed);

    // The same claim after line 12 is the reviewer misreading the scene.
    let mistaken = leak.clone().at(20);
    assert!(matches!(
        story.substantiate(&mistaken),
        Substantiation::Refuted(_)
    ));

    // And by scene three the knowledge is simply hers.
    let stale = Ticket::new(TicketCode::KnowledgeLeak, sid("s3"), "she could not know").claiming(
        Claim::Knowledge {
            character: cid("mara"),
            fact: fid("harek-broke-it"),
        },
    );
    assert!(matches!(
        story.substantiate(&stale),
        Substantiation::Refuted(_)
    ));
}

#[test]
fn a_claim_about_something_that_does_not_exist_is_refuted() {
    let story = fixture();
    let bogus =
        Ticket::new(TicketCode::KnowledgeLeak, sid("s2"), "leak").claiming(Claim::Knowledge {
            character: cid("mara"),
            fact: fid("the-moon-is-cheese"),
        });
    assert!(matches!(
        story.substantiate(&bogus),
        Substantiation::Refuted(_)
    ));
}

#[test]
fn revising_a_scene_marks_only_its_downstream_scenes_dirty() {
    let story = fixture();

    let dirty = story.dirty_after(&sid("s2"));
    assert_eq!(
        dirty,
        vec![sid("s3"), sid("s4")],
        "s3 depends on s2 and s4 depends on s3; s5 depends on s1 and is untouched"
    );

    assert_eq!(story.revision_blast_radius(&sid("s4")), 0);
}

#[test]
fn a_planned_scene_is_not_dirty_because_it_has_no_prose_to_recheck() {
    let story = fixture();
    let dirty = story.dirty_after(&sid("s1"));
    assert!(
        !dirty.contains(&sid("s5")),
        "s5 is still a beat on the board; there is no prose to recheck"
    );
}

#[test]
fn causality_that_runs_backwards_is_a_structural_error() {
    let mut story = fixture();
    let first = story.scenes.iter_mut().find(|s| s.id == sid("s1")).unwrap();
    first.dependencies.push(CausalLink {
        kind: CausalKind::Therefore,
        scene: sid("s4"),
        note: String::new(),
    });
    let errors = story.check();
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, canonwright_story::StoryError::CausalityOrder { .. }))
    );
}

#[test]
fn a_scene_that_changes_nothing_is_flagged_but_never_blocked() {
    let mut story = fixture();
    let three = story.scenes.iter_mut().find(|s| s.id == sid("s3")).unwrap();
    three.value_end = three.value_start.clone();

    let tickets = story.lint();
    let ticket = tickets
        .iter()
        .find(|ticket| ticket.code == TicketCode::NoValueShift && ticket.scene == sid("s3"))
        .expect("an unchanged scene should be flagged");
    assert!(!ticket.blocking, "lint never blocks a commit");
    assert_eq!(story.substantiate(ticket), Substantiation::Confirmed);
}

#[test]
fn a_plant_with_no_payoff_goes_cold_after_the_grace_window() {
    let mut story = fixture();
    story.threads.insert(
        tid("the-letter"),
        Thread {
            kind: ThreadKind::Gun,
            label: "The unopened letter".to_owned(),
            opened_in: sid("s1"),
            promised_payoff: None,
            note: String::new(),
        },
    );
    // Five scenes after the plant is inside the window; seven is not.
    story
        .scenes
        .push(depends_on(scene("s6", 6, SceneStatus::Drafted), "s5"));
    assert!(
        !story
            .lint()
            .iter()
            .any(|ticket| ticket.code == TicketCode::DanglingSetup),
        "five scenes of silence is not yet cold"
    );

    story
        .scenes
        .push(depends_on(scene("s7", 7, SceneStatus::Drafted), "s6"));
    let cold = story
        .lint()
        .into_iter()
        .find(|ticket| ticket.code == TicketCode::DanglingSetup)
        .expect("six scenes of silence is a dangling setup");
    assert_eq!(cold.scene, sid("s1"));
    assert_eq!(story.substantiate(&cold), Substantiation::Confirmed);
}

#[test]
fn a_thread_that_pays_off_stops_being_owed() {
    let mut story = fixture();
    story.threads.insert(
        tid("the-letter"),
        Thread {
            kind: ThreadKind::Gun,
            label: "The unopened letter".to_owned(),
            opened_in: sid("s1"),
            promised_payoff: Some(sid("s2")),
            note: String::new(),
        },
    );
    let two = story.scenes.iter_mut().find(|s| s.id == sid("s2")).unwrap();
    two.delta.changes.push(SceneChange::at(
        40,
        Change::ThreadPaidOff {
            thread: tid("the-letter"),
        },
    ));

    let state = story.state_at(&sid("s2"));
    assert_eq!(
        state.thread(&tid("the-letter")).status,
        ThreadStatus::PaidOff
    );
    assert!(
        !story
            .lint()
            .iter()
            .any(|ticket| ticket.code == TicketCode::UnearnedResolution)
    );
}

#[test]
fn a_promised_payoff_that_never_lands_is_flagged() {
    let mut story = fixture();
    story.threads.insert(
        tid("the-letter"),
        Thread {
            kind: ThreadKind::Gun,
            label: "The unopened letter".to_owned(),
            opened_in: sid("s1"),
            promised_payoff: Some(sid("s2")),
            note: String::new(),
        },
    );
    let unearned = story
        .lint()
        .into_iter()
        .find(|ticket| ticket.code == TicketCode::UnearnedResolution)
        .expect("the promise was made and not kept");
    assert_eq!(unearned.scene, sid("s2"));
}

#[test]
fn a_character_who_appears_after_dying_is_a_continuity_break() {
    let mut story = fixture();
    let two = story.scenes.iter_mut().find(|s| s.id == sid("s2")).unwrap();
    two.present.push(cid("harek"));
    two.delta.changes.push(SceneChange::unplaced(Change::Death {
        character: cid("harek"),
    }));
    let three = story.scenes.iter_mut().find(|s| s.id == sid("s3")).unwrap();
    three.present.push(cid("harek"));

    let tickets = story.lint();
    assert!(
        tickets
            .iter()
            .any(|ticket| ticket.code == TicketCode::ContinuityBreak && ticket.scene == sid("s3")),
        "{tickets:?}"
    );
}

#[test]
fn the_scene_brief_withholds_the_future() {
    let story = fixture();
    let context = story.context_for(&sid("s2")).unwrap();

    assert!(context.markdown.contains("Scene 2"));
    assert!(
        !context.markdown.contains("Scene 3"),
        "a later scene is not a premise"
    );
    assert!(
        !context.markdown.contains("Harek ordered the ford burned"),
        "she has not learned it yet at the top of the scene"
    );
    assert!(
        context
            .markdown
            .contains("The truce was broken at the ford")
    );
    assert_eq!(context.withheld.later_scenes, 3);
    assert_eq!(context.withheld.unrevealed_facts, 1);
}

#[test]
fn relationships_fold_forward_and_clamp() {
    let mut story = fixture();
    let one = story.scenes.iter_mut().find(|s| s.id == sid("s1")).unwrap();
    one.delta
        .changes
        .push(SceneChange::unplaced(Change::Relationship {
            from: cid("mara"),
            to: TargetRef::character(cid("harek")),
            axis: Axis::Trust,
            value: 4,
            note: "he held the ford".to_owned(),
        }));
    let two = story.scenes.iter_mut().find(|s| s.id == sid("s2")).unwrap();
    two.delta
        .changes
        .push(SceneChange::unplaced(Change::Relationship {
            from: cid("mara"),
            to: TargetRef::character(cid("harek")),
            axis: Axis::Trust,
            value: -9,
            note: "he burned it".to_owned(),
        }));

    let after_one = story.state_at(&sid("s1"));
    assert_eq!(
        after_one
            .relationship(&cid("mara"), &TargetRef::character(cid("harek")))
            .trust,
        4
    );
    let after_two = story.state_at(&sid("s2"));
    let record = after_two.relationship(&cid("mara"), &TargetRef::character(cid("harek")));
    assert_eq!(record.trust, -5, "one scene cannot exceed the axis");
    assert_eq!(record.note, "he burned it");
}

#[test]
fn a_story_round_trips_through_json_without_losing_the_timeline() {
    let story = fixture();
    let encoded = story.to_json().unwrap();
    let decoded = Story::from_json(&encoded).unwrap();
    assert_eq!(story, decoded);
    assert_eq!(decoded.state_at(&sid("s2")), story.state_at(&sid("s2")));
}

#[test]
fn the_fixture_is_structurally_sound() {
    let story = fixture();
    let errors = story.check();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn a_hard_rule_claim_is_checkable_but_soft_lore_is_not() {
    let mut story = fixture();
    story.rules.insert(
        rid("salt-is-scarce"),
        Rule {
            kind: RuleKind::Hard,
            statement: "No salt leaves the valley except by the duke's seal.".to_owned(),
            scope: None,
        },
    );
    story.rules.insert(
        rid("the-ford-is-haunted"),
        Rule {
            kind: RuleKind::Soft,
            statement: "Locals say the ford is haunted.".to_owned(),
            scope: None,
        },
    );

    let hard = Ticket::new(
        TicketCode::RuleViolation,
        sid("s2"),
        "salt moved without a seal",
    )
    .claiming(Claim::Rule {
        rule: rid("salt-is-scarce"),
    });
    assert_eq!(story.substantiate(&hard), Substantiation::Confirmed);

    let soft = Ticket::new(TicketCode::RuleViolation, sid("s2"), "the ford is haunted").claiming(
        Claim::Rule {
            rule: rid("the-ford-is-haunted"),
        },
    );
    assert!(matches!(
        story.substantiate(&soft),
        Substantiation::Refuted(_)
    ));
}

#[test]
fn an_uncheckable_ticket_says_so_instead_of_guessing() {
    let story = fixture();
    let taste = Ticket::new(
        TicketCode::OnTheNose,
        sid("s2"),
        "she says exactly what she feels",
    );
    assert_eq!(story.substantiate(&taste), Substantiation::Uncheckable);
}

/// The state type is public API; a prompt builder outside this crate should be
/// able to read it without reaching into internals.
fn _state_is_readable(state: &StoryState) -> usize {
    state.characters.len() + state.threads.len()
}
