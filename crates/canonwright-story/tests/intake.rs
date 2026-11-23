use canonwright_story::intake::{Intake, Mode, infer_pov, infer_tense};
use canonwright_story::prelude::*;

fn first_person_past() -> String {
    "I was tired. I had walked for hours and I said nothing to anyone. \
     I went to the ford and I could not cross it. I waited. I was alone, \
     and my hands were cold. I had no map, and I said so."
        .repeat(4)
}

fn third_person_present() -> String {
    "She is tired. He is late and she says nothing to him. They are at the ford \
     and it is closed. She waits. He is alone and his hands are cold. She has no \
     map, and she says so."
        .repeat(4)
}

#[test]
fn the_interview_is_the_same_every_time() {
    let first = Intake::new_manuscript();
    let second = Intake::new_manuscript();
    assert_eq!(first, second, "the script must be deterministic");

    let ids: Vec<&str> = first.questions.iter().map(|q| q.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "title",
            "author",
            "genre",
            "logline",
            "pov_voice",
            "tense",
            "protagonist",
            "want",
            "need",
            "central_question",
            "hard_rule",
        ]
    );
}

#[test]
fn only_the_title_is_required_and_everything_else_can_be_skipped() {
    let intake = Intake::new_manuscript();
    let required: Vec<&str> = intake
        .questions
        .iter()
        .filter(|question| question.required)
        .map(|question| question.id.as_str())
        .collect();
    assert_eq!(required, vec!["title"]);
    assert_eq!(intake.skippable_count(), intake.questions.len() - 1);
    assert_eq!(
        intake.outstanding().len(),
        1,
        "only the title is outstanding"
    );
}

#[test]
fn an_import_answers_what_the_document_can_and_asks_the_rest() {
    let intake = Intake::from_document(&first_person_past(), Some("The Salt Road"));
    assert_eq!(intake.mode, Mode::Imported);
    assert_eq!(intake.value("title"), Some("The Salt Road"));
    assert_eq!(intake.value("pov_voice"), Some("first person"));
    assert_eq!(intake.value("tense"), Some("past"));

    let inferred: Vec<&str> = intake
        .questions
        .iter()
        .filter(|question| question.inferred)
        .map(|question| question.id.as_str())
        .collect();
    assert_eq!(inferred, vec!["title", "pov_voice", "tense"]);

    // The things a document cannot know are still asked.
    assert_eq!(intake.value("genre"), None);
    assert_eq!(intake.value("logline"), None);
    assert_eq!(intake.value("hard_rule"), None);

    assert!(
        intake
            .derived
            .iter()
            .any(|fact| fact.label == "words in the draft"),
        "{:?}",
        intake.derived
    );
    assert!(
        intake.outstanding().is_empty(),
        "the document supplied the title"
    );
}

#[test]
fn a_mixed_document_admits_it_cannot_tell() {
    let neutral = "The room was quiet. The door opened. A man entered.".repeat(3);
    assert_eq!(infer_pov(&neutral), None);
    assert_eq!(infer_tense(&neutral), None);

    assert_eq!(infer_pov(&first_person_past()), Some("first person"));
    assert_eq!(infer_tense(&first_person_past()), Some("past"));
    assert_eq!(infer_pov(&third_person_present()), Some("third limited"));
    assert_eq!(infer_tense(&third_person_present()), Some("present"));
}

#[test]
fn an_author_answer_overrides_an_inferred_one() {
    let mut intake = Intake::from_document(&first_person_past(), Some("Draft"));
    assert_eq!(intake.value("pov_voice"), Some("first person"));

    intake
        .answer("pov_voice", "third limited")
        .expect("known question");
    assert_eq!(intake.value("pov_voice"), Some("third limited"));
    let question = intake.question("pov_voice").expect("question");
    assert!(!question.inferred, "a typed answer is not an inference");
}

#[test]
fn an_unknown_question_is_rejected_rather_than_dropped() {
    let mut intake = Intake::new_manuscript();
    let error = intake
        .answer("favourite_colour", "blue")
        .expect_err("unknown id");
    assert!(error.to_string().contains("favourite_colour"), "{error}");
}

#[test]
fn answers_land_in_the_story() {
    let mut intake = Intake::new_manuscript();
    intake.answer("title", "The Salt Road").expect("title");
    intake.answer("genre", "fantasy").expect("genre");
    intake.answer("pov_voice", "third limited").expect("voice");
    intake.answer("tense", "past").expect("tense");
    intake
        .answer("protagonist", "Mara Vale")
        .expect("protagonist");
    intake
        .answer("want", "Get the carts through the ford")
        .expect("want");
    intake
        .answer("need", "Admit she left the valley out of fear")
        .expect("need");
    intake
        .answer("central_question", "Who burned the ford?")
        .expect("question");
    intake
        .answer("hard_rule", "No salt crosses without the duke's seal.")
        .expect("rule");

    let mut story = Story::default();
    intake.apply(&mut story);

    assert_eq!(story.meta.title, "The Salt Road");
    assert_eq!(story.meta.genre.as_deref(), Some("fantasy"));
    assert_eq!(story.meta.pov_voice.as_deref(), Some("third limited"));
    assert_eq!(story.meta.tense.as_deref(), Some("past"));
    assert_eq!(story.meta.premise.as_deref(), Some("Who burned the ford?"));

    let mara = CharacterId::new("mara-vale").expect("slug");
    assert_eq!(story.meta.pov, Some(mara.clone()));
    let character = story.characters.get(&mara).expect("protagonist exists");
    assert_eq!(character.name, "Mara Vale");
    assert_eq!(character.want, "Get the carts through the ford");
    assert!(character.need.contains("fear"));

    assert!(
        story
            .rules
            .contains_key(&RuleId::new("world-rule-1").expect("slug"))
    );
    // A blank manuscript has no scene, so the premise stays in the meta.
    assert!(story.threads.is_empty());
    assert!(story.check().is_empty(), "{:?}", story.check());
}

#[test]
fn the_central_question_becomes_a_thread_once_a_scene_exists() {
    let mut intake = Intake::new_manuscript();
    intake.answer("title", "The Salt Road").expect("title");
    intake
        .answer("central_question", "Who burned the ford?")
        .expect("question");

    let mut story = Story::default();
    story.scenes.push(Scene {
        id: SceneId::new("s1").expect("slug"),
        index: 1,
        pov: Some(CharacterId::new("narrator").expect("slug")),
        ..Scene::default()
    });
    story.characters.insert(
        CharacterId::new("narrator").expect("slug"),
        Character::default(),
    );
    intake.apply(&mut story);

    let thread = story
        .threads
        .get(&ThreadId::new("central-question").expect("slug"))
        .expect("thread planted");
    assert_eq!(thread.opened_in, SceneId::new("s1").expect("slug"));
    assert_eq!(thread.label, "Who burned the ford?");
    assert!(story.check().is_empty(), "{:?}", story.check());
}

#[test]
fn skipping_everything_that_can_be_skipped_is_not_an_error() {
    let intake = Intake::new_manuscript();
    let mut story = Story::default();
    intake.apply(&mut story);
    assert_eq!(story.meta.title, "");
    assert!(story.check().is_empty(), "an empty story is a valid start");
}
