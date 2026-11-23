use canonwright_story::intake::Intake;
use canonwright_story::prelude::*;

fn rules_intake() -> Intake {
    let mut intake = Intake::new_rules();
    intake.answer("title", "Deeds of Legend").expect("title");
    intake.answer("system", "Deeds of Legend").expect("system");
    intake
        .answer(
            "core_mechanic",
            "Roll 2d10 and add the trait; 11+ succeeds.",
        )
        .expect("core");
    intake.answer("dice", "2d10").expect("dice");
    intake
        .answer(
            "balance_rule",
            "No trait may exceed 5 at character creation.",
        )
        .expect("balance");
    intake
}

#[test]
fn a_rulebook_is_interviewed_differently_from_a_novel() {
    let narrative = Intake::new_manuscript();
    let rules = Intake::new_rules();

    let novel_ids: Vec<&str> = narrative.questions.iter().map(|q| q.id.as_str()).collect();
    let rules_ids: Vec<&str> = rules.questions.iter().map(|q| q.id.as_str()).collect();

    assert_ne!(novel_ids, rules_ids, "the two books are not the same book");
    assert_eq!(
        rules_ids,
        vec![
            "title",
            "author",
            "system",
            "genre",
            "tone",
            "core_mechanic",
            "dice",
            "balance_rule",
        ]
    );
    assert!(
        !rules_ids.contains(&"protagonist"),
        "a rulebook has no protagonist"
    );
    assert_eq!(rules.kind, WorkKind::Rules);

    let required: Vec<&str> = rules
        .questions
        .iter()
        .filter(|question| question.required)
        .map(|question| question.id.as_str())
        .collect();
    assert_eq!(required, vec!["title"], "still only the title is required");
}

#[test]
fn rules_answers_land_as_hard_rules_the_reviewers_can_check() {
    let intake = rules_intake();
    let mut story = Story::default();
    intake.apply(&mut story);

    assert_eq!(story.meta.kind, WorkKind::Rules);
    assert_eq!(story.meta.title, "Deeds of Legend");
    assert_eq!(story.meta.system.as_deref(), Some("Deeds of Legend"));
    assert_eq!(story.rules.len(), 3);
    assert!(
        story
            .rules
            .values()
            .all(|rule| rule.kind == RuleKind::Hard && rule.scope.as_deref() == Some("system")),
        "{:?}",
        story.rules
    );
    assert!(story.characters.is_empty(), "no cast was invented");
    assert!(story.check().is_empty(), "{:?}", story.check());
}

#[test]
fn a_rulebook_chapter_needs_no_point_of_view() {
    let mut story = Story::default();
    story.meta.kind = WorkKind::Rules;
    story.scenes.push(Scene {
        id: SceneId::new("ch1").expect("slug"),
        index: 1,
        title: "Chapter 1: The Core Mechanic".to_owned(),
        pov: None,
        status: SceneStatus::Drafted,
        ..Scene::default()
    });

    assert!(
        story.check().is_empty(),
        "a chapter without a viewpoint is not an error: {:?}",
        story.check()
    );

    let context = story
        .context_for(&SceneId::new("ch1").expect("slug"))
        .expect("brief");
    assert!(
        context.markdown.contains("(none"),
        "the brief should say so rather than inventing a viewpoint: {}",
        context.markdown
    );
}

#[test]
fn narrative_lint_stays_out_of_a_rulebook() {
    let mut story = Story::default();
    story.meta.kind = WorkKind::Rules;
    story.scenes.push(Scene {
        id: SceneId::new("ch1").expect("slug"),
        index: 1,
        pov: None,
        value_start: String::new(),
        value_end: String::new(),
        dependencies: Vec::new(),
        status: SceneStatus::Drafted,
        ..Scene::default()
    });

    let tickets = story.lint();
    assert!(
        !tickets
            .iter()
            .any(|ticket| ticket.code == TicketCode::NoValueShift),
        "a rulebook chapter has no value to shift: {tickets:?}"
    );
}

#[test]
fn a_hybrid_book_still_gets_the_narrative_checks() {
    let mut story = Story::default();
    story.meta.kind = WorkKind::Hybrid;
    story.characters.insert(
        CharacterId::new("mara").expect("slug"),
        Character::default(),
    );
    story.scenes.push(Scene {
        id: SceneId::new("s1").expect("slug"),
        index: 1,
        pov: Some(CharacterId::new("mara").expect("slug")),
        status: SceneStatus::Drafted,
        ..Scene::default()
    });

    assert!(
        story
            .lint()
            .iter()
            .any(|ticket| ticket.code == TicketCode::NoValueShift),
        "a campaign book has scenes, and they still have to change something"
    );
}

#[test]
fn a_cross_reference_must_resolve() {
    let mut story = Story::default();
    story.meta.kind = WorkKind::Rules;
    story.rules.insert(
        RuleId::new("core-resolution").expect("slug"),
        Rule {
            kind: RuleKind::Hard,
            statement: "Roll 2d10.".to_owned(),
            scope: Some("system".to_owned()),
        },
    );
    story.entities.insert(
        EntityId::new("fighter").expect("slug"),
        Entity {
            kind: EntityKind::Class,
            name: "Fighter".to_owned(),
            summary: "A trained combatant.".to_owned(),
            fields: [("Traits".to_owned(), "Might, Edge".to_owned())]
                .into_iter()
                .collect(),
            references: vec![Reference {
                to: "core-resolution".to_owned(),
                note: "how the class resolves attacks".to_owned(),
            }],
        },
    );
    assert!(story.check().is_empty(), "{:?}", story.check());

    // Point it at something that does not exist.
    story
        .entities
        .get_mut(&EntityId::new("fighter").expect("slug"))
        .expect("fighter")
        .references
        .push(Reference {
            to: "grapple".to_owned(),
            note: "see the grapple procedure".to_owned(),
        });
    let errors = story.check();
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("grapple")),
        "a dangling reference is the rulebook's compile error: {errors:?}"
    );
}

#[test]
fn a_class_without_numbers_is_flagged_but_not_blocked() {
    let mut story = Story::default();
    story.meta.kind = WorkKind::Rules;
    story.entities.insert(
        EntityId::new("fighter").expect("slug"),
        Entity {
            kind: EntityKind::Class,
            name: "Fighter".to_owned(),
            ..Entity::default()
        },
    );

    let tickets = story.lint();
    let ticket = tickets
        .iter()
        .find(|ticket| ticket.code == TicketCode::IncompleteEntity)
        .expect("an empty class is flagged");
    assert!(!ticket.blocking);
    assert_eq!(story.substantiate(ticket), Substantiation::Uncheckable);
}

#[test]
fn a_reference_may_name_an_entity_a_rule_or_a_scene() {
    let mut story = Story::default();
    story.meta.kind = WorkKind::Rules;
    story.scenes.push(Scene {
        id: SceneId::new("ch1").expect("slug"),
        index: 1,
        pov: None,
        ..Scene::default()
    });
    story.rules.insert(
        RuleId::new("core-resolution").expect("slug"),
        Rule {
            kind: RuleKind::Hard,
            statement: "Roll 2d10.".to_owned(),
            scope: None,
        },
    );
    story.entities.insert(
        EntityId::new("fighter").expect("slug"),
        Entity {
            kind: EntityKind::Class,
            name: "Fighter".to_owned(),
            summary: "A trained combatant.".to_owned(),
            ..Entity::default()
        },
    );

    assert!(story.resolves("ch1"));
    assert!(story.resolves("core-resolution"));
    assert!(story.resolves("fighter"));
    assert!(!story.resolves("grapple"));
}
