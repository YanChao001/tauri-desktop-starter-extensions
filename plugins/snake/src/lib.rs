//! The isolated UI controls the arena; only a bounded Wasm replay can save a score.
mod arena;

use arena::{Checkpoint, Difficulty, MAX_TICKS, Summary, replay, summary};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const STORAGE_KEY: &str = "snake-records";
const MAX_CHECKPOINT_BYTES: usize = 200_000;
const MAX_STATE_BYTES: usize = 224_000;
// Storage is an untrusted boundary. These generous caps bound imported counters;
// every new score is calculated by replay rather than accepted from this storage.
const MAX_RECORD_SCORE: u32 = 100_000;
const MAX_RECORD_FOODS: u32 = MAX_TICKS * 700;
const MAX_RECORD_KILLS: u32 = MAX_TICKS * 12;

fn difficulty_name(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}

fn run_id(checkpoint: &Checkpoint) -> String {
    // A stable local identity deduplicates saves, including after changing skin
    // or audio preferences. This is not a signature or an online anti-cheat token.
    format!(
        "{:08x}{}",
        checkpoint.seed,
        difficulty_code(checkpoint.difficulty)
    )
}

fn difficulty_code(difficulty: Difficulty) -> char {
    match difficulty {
        Difficulty::Easy => '0',
        Difficulty::Normal => '1',
        Difficulty::Hard => '2',
    }
}

fn valid_run_id(entry: &ScoreEntry, legacy: bool) -> bool {
    if !entry
        .run_id
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return false;
    }
    // Schema 3 hashes cannot recover their seeds. Keep their eight-character
    // historical identity distinct from every new, exact nine-character ID.
    match entry.run_id.len() {
        8 => true,
        9 if !legacy => {
            u32::from_str_radix(&entry.run_id[..8], 16).is_ok_and(|seed| seed != 0)
                && entry.run_id.as_bytes()[8] == difficulty_code(entry.difficulty) as u8
        }
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Records {
    schema: u8,
    best: BTreeMap<String, u32>,
    scores: Vec<ScoreEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ScoreEntry {
    score: u32,
    kills: u32,
    foods: u32,
    ticks: u32,
    difficulty: Difficulty,
    run_id: String,
}

impl Default for Records {
    fn default() -> Self {
        Self {
            schema: 4,
            best: BTreeMap::new(),
            scores: vec![],
        }
    }
}

fn records(raw: Option<&Value>) -> Result<Records, String> {
    let Some(raw) = raw else {
        return Ok(Records::default());
    };
    let mut result: Records = serde_json::from_value(raw.clone())
        .map_err(|_| "Invalid or future snake records / 纪录损坏或版本不受支持，已保留原件")?;
    if result.schema == 3 {
        validate_records_schema(&result, true)?;
        // Read-only until an explicit save emits the usual revision CAS.
        result.schema = 4;
    }
    validate_records(&result)?;
    Ok(result)
}

fn validate_records(result: &Records) -> Result<(), String> {
    validate_records_schema(result, false)
}

fn validate_records_schema(result: &Records, legacy: bool) -> Result<(), String> {
    if result.schema != if legacy { 3 } else { 4 }
        || result.best.len() > 3
        || result.best.iter().any(|(difficulty, score)| {
            !["easy", "normal", "hard"].contains(&difficulty.as_str()) || *score > MAX_RECORD_SCORE
        })
        || result.scores.len() > 10
        || result.scores.iter().enumerate().any(|(index, entry)| {
            entry.score == 0
                || entry.score > MAX_RECORD_SCORE
                || entry.kills > MAX_RECORD_KILLS
                || entry.foods > MAX_RECORD_FOODS
                || entry.ticks == 0
                || entry.ticks > MAX_TICKS
                || result
                    .best
                    .get(difficulty_name(entry.difficulty))
                    .is_none_or(|best| *best < entry.score)
                || !valid_run_id(entry, legacy)
                || result.scores[..index]
                    .iter()
                    .any(|previous| previous.run_id == entry.run_id)
                || (index > 0 && result.scores[index - 1].score < entry.score)
        })
    {
        return Err("Invalid or future snake records / 纪录损坏或版本不受支持，已保留原件".into());
    }
    Ok(())
}

fn update_records(stored: &mut Records, checkpoint: &Checkpoint, result: &Summary) {
    let best = stored
        .best
        .entry(difficulty_name(checkpoint.difficulty).into())
        .or_default();
    *best = (*best).max(result.max_score);
    // An unplayed menu checkpoint may persist its preferences but is not a run.
    if result.ticks == 0 || result.max_score == 0 {
        return;
    }
    let entry = ScoreEntry {
        score: result.max_score,
        kills: result.kills,
        foods: result.foods,
        ticks: result.ticks,
        difficulty: checkpoint.difficulty,
        run_id: run_id(checkpoint),
    };
    match stored
        .scores
        .iter_mut()
        .find(|existing| existing.run_id == entry.run_id)
    {
        Some(existing) if existing.score < entry.score => *existing = entry,
        Some(_) => {}
        None => stored.scores.push(entry),
    }
    // Stable sorting preserves the order of tied runs and duplicate saves.
    stored
        .scores
        .sort_by_key(|entry| std::cmp::Reverse(entry.score));
    stored.scores.truncate(10);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    revision: i32,
    mode: String,
    best: u32,
    expected: Records,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u8,
    game: Checkpoint,
    notice: String,
    pending: Option<Pending>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schema: 3,
            game: Checkpoint::default(),
            notice: "ready".into(),
            pending: None,
        }
    }
}

fn tr<'a>(zh: bool, cn: &'a str, en: &'a str) -> &'a str {
    if zh { cn } else { en }
}

pub fn process(request: Value) -> Result<Value, String> {
    if request["version"] != 2 || !matches!(request["locale"].as_str(), Some("zh-CN" | "en")) {
        return Err("Unsupported request / 不支持的请求".into());
    }
    let zh = request["locale"] == "zh-CN";
    let raw = request["state"].as_str().ok_or("Missing state")?;
    if raw.len() > MAX_STATE_BYTES {
        return Err("Snake state too large".into());
    }
    let mut state: State = if raw.is_empty() {
        State::default()
    } else {
        serde_json::from_str(raw).map_err(|_| "Invalid snake state")?
    };
    if state.schema != 3
        || ![
            "ready",
            "checked",
            "saved",
            "conflict",
            "failed",
            "omitted",
            "cancelled",
            "no-storage",
        ]
        .contains(&state.notice.as_str())
    {
        return Err("Invalid snake state".into());
    }
    let action = match &request["action"] {
        Value::Null => "",
        Value::String(action) => action,
        _ => return Err("Invalid snake action / 无效的贪吃蛇操作".into()),
    };
    if state.pending.is_some() != (action == "host-result") {
        return Err("Unexpected or missing storage receipt".into());
    }
    if matches!(action, "validate" | "save-best") {
        let checkpoint = request["fields"]["checkpoint"]
            .as_str()
            .ok_or("Missing snake checkpoint")?;
        if checkpoint.len() > MAX_CHECKPOINT_BYTES {
            return Err("Snake checkpoint too large".into());
        }
        state.game = serde_json::from_str(checkpoint).map_err(|_| "Invalid snake checkpoint")?;
    }
    let result = summary(&replay(&state.game)?);
    let granted = request["host"]["permissions"]
        .as_array()
        .is_some_and(|list| list.iter().any(|value| value == "storage"));
    let snapshot = &request["host"]["storage"];
    // Damaged optional records disable saving without preventing an offline game.
    // The host retains their original bytes; this plugin never resets them.
    let storage = || -> Result<(i32, Records), String> {
        if !granted {
            return Ok((0, Records::default()));
        }
        let revision = snapshot["revision"]
            .as_i64()
            .filter(|revision| (0..i32::MAX as i64).contains(revision))
            .ok_or("Invalid storage revision")? as i32;
        let values = snapshot["values"]
            .as_object()
            .ok_or("Invalid storage values")?;
        Ok((revision, records(values.get(STORAGE_KEY))?))
    };
    let (revision, mut stored, mut storage_error) = match storage() {
        Ok((revision, stored)) => (revision, stored, None),
        Err(_) => (
            0,
            Records::default(),
            Some(tr(
                zh,
                "已保留原始纪录，但其版本、内容或存储快照不受支持；保存已禁用，仍可离线游玩和导出会话。",
                "Original records were retained, but their version, contents or storage snapshot are unsupported. Saving is disabled; offline play and session export remain available.",
            )),
        ),
    };
    let mode = difficulty_name(state.game.difficulty);
    let mut effects = vec![];
    match action {
        "" => {}
        "validate" => state.notice = "checked".into(),
        "save-best" if !granted => state.notice = "no-storage".into(),
        "save-best" if storage_error.is_some() => {
            return Err(storage_error.unwrap().into());
        }
        "save-best" => {
            update_records(&mut stored, &state.game, &result);
            validate_records(&stored)?;
            let best = stored.best[mode];
            state.pending = Some(Pending {
                revision,
                mode: mode.into(),
                best,
                expected: stored.clone(),
            });
            effects.push(json!({"kind":"storage","id":"save-best","expectedRevision":revision,"changes":{STORAGE_KEY:stored}}));
        }
        "host-result" => {
            let pending = state.pending.take().ok_or("Missing pending result")?;
            validate_records(&pending.expected)?;
            if pending.revision < 0
                || pending.revision == i32::MAX
                || pending.mode != mode
                || pending.best < result.max_score
                || pending.best > MAX_RECORD_SCORE
                || pending.expected.best.get(mode) != Some(&pending.best)
            {
                return Err("Invalid pending snake record".into());
            }
            // Reapplying the replay must not improve the claimed expected data.
            // This also catches an omitted or substituted current leaderboard row.
            let mut checked = pending.expected.clone();
            update_records(&mut checked, &state.game, &result);
            if checked != pending.expected {
                return Err("Invalid pending snake leaderboard".into());
            }
            let results = request["results"]
                .as_array()
                .ok_or("Missing effect results")?;
            if results.len() != 1 || results[0]["id"] != "save-best" {
                return Err("Invalid snake storage receipt".into());
            }
            state.notice = if results[0]["error"].is_null() {
                let value = &results[0]["value"];
                let values = value["values"]
                    .as_object()
                    .ok_or("Invalid snake success receipt")?;
                // Missing storage keys are not equivalent to empty records here.
                // A receipt must confirm the exact current-schema write, not
                // merely data that could be upgraded into the expected value.
                let saved: Records = serde_json::from_value(
                    values
                        .get(STORAGE_KEY)
                        .ok_or("Missing snake success records")?
                        .clone(),
                )
                .map_err(|_| "Invalid snake success records")?;
                validate_records(&saved)?;
                if value["revision"].as_i64() != Some(i64::from(pending.revision) + 1)
                    || saved != pending.expected
                {
                    return Err("Invalid snake success receipt".into());
                }
                // The injected snapshot can predate the confirmed write.
                stored = saved;
                storage_error = None;
                "saved"
            } else {
                if !results[0]["value"].is_null() {
                    return Err("Invalid snake failure receipt".into());
                }
                match results[0]["error"]["code"].as_str() {
                    Some("CONFLICT") => "conflict",
                    Some("INPUT_TOO_LONG") => "omitted",
                    Some("USER_CANCELLED") => "cancelled",
                    Some(_) => "failed",
                    None => return Err("Invalid snake failure receipt".into()),
                }
            }
            .into();
        }
        _ => return Err("Unknown snake action / 未知的贪吃蛇操作".into()),
    }
    let description = storage_error.unwrap_or(match state.notice.as_str() {
        "checked" => tr(
            zh,
            "进度已由 Wasm 重放核验。",
            "The checkpoint was replayed and validated by Wasm.",
        ),
        "saved" => tr(
            zh,
            "当前难度纪录已保存。",
            "The best score for this difficulty was saved.",
        ),
        "conflict" => tr(
            zh,
            "另一窗口更新了纪录；对局保留，请审阅后手动再次保存。",
            "Another window changed records. Your game is retained; review and save again manually.",
        ),
        "omitted" => tr(
            zh,
            "保存操作已执行，响应被省略；对局保留，不要自动重试。",
            "The save was executed but its response was omitted. Your game is retained; do not automatically retry.",
        ),
        "cancelled" => tr(
            zh,
            "已停止等待保存结果；对局保留，写入可能已完成，请刷新纪录后再决定是否保存。",
            "Waiting for the save was cancelled. Your game is retained; the write may have completed. Refresh records before deciding to save again.",
        ),
        "failed" => tr(
            zh,
            "保存未完成；对局保留，未自动重试。",
            "The save did not complete. Your game is retained; no automatic retry was made.",
        ),
        "no-storage" => tr(
            zh,
            "未授权私有存储，仍可完整游玩和导出会话。",
            "Private storage is not granted. Play and session export remain available.",
        ),
        _ => tr(
            zh,
            "鼠标或摇杆自由转向；按住空格、Shift 或鼠标加速，Esc 暂停。离开后自动暂停，返回时手动继续。",
            "Steer freely with the mouse or joystick. Hold Space, Shift or the mouse to boost; Escape pauses. Leaving pauses the arena; resume manually when you return.",
        ),
    });
    let best = stored.best.get(mode).copied().unwrap_or(0);
    let record_rows: Vec<_> = stored
        .best
        .iter()
        .map(|(mode, best)| vec![mode.clone(), best.to_string()])
        .collect();
    let score_rows: Vec<_> = stored
        .scores
        .iter()
        .map(|entry| {
            vec![
                entry.score.to_string(),
                entry.kills.to_string(),
                entry.foods.to_string(),
                entry.ticks.to_string(),
                difficulty_name(entry.difficulty).into(),
                entry.run_id.clone(),
            ]
        })
        .collect();
    let mut blocks = vec![
        json!({"kind":"metrics","items":[
            {"label":tr(zh,"最高长度","Score"),"value":result.max_score.to_string()},
            {"label":tr(zh,"难度纪录","Mode best"),"value":best.to_string()},
            {"label":tr(zh,"步数","Ticks"),"value":result.ticks.to_string()}
        ]}),
        json!({"kind":"table","columns":["mode","best"],"rows":record_rows}),
        json!({"kind":"table","columns":["Score","Kills","Foods","Ticks","Difficulty","Run"],"rows":score_rows}),
    ];
    if let Some(error) = storage_error {
        blocks.push(json!({"kind":"markdown","text":error}));
    }
    let serialized_state =
        serde_json::to_string(&state).map_err(|_| "Could not encode snake state")?;
    if serialized_state.len() > MAX_STATE_BYTES {
        return Err("Snake state too large".into());
    }
    Ok(json!({
        "version":2,"display":tr(zh,"贪吃蛇","Snake"),"description":description,
        "state":serialized_state,
        "fields":[{"id":"checkpoint","label":tr(zh,"对局进度","Game checkpoint"),"value":serde_json::to_string(&state.game).map_err(|_| "Could not encode checkpoint")?,"kind":"textarea","maxBytes":MAX_CHECKPOINT_BYTES}],
        "groups":[{"columns":2,"actions":[
            {"id":"validate","label":tr(zh,"核验进度","Validate checkpoint"),"disabled":false,"primary":false},
            {"id":"save-best","label":tr(zh,"保存纪录","Save best score"),"disabled":!granted || storage_error.is_some(),"primary":true}
        ]}],
        "blocks":blocks,"effects":effects,"artifacts":[],"errors":{}
    }))
}

#[cfg(target_arch = "wasm32")]
#[extism_pdk::plugin_fn]
pub fn play(input: extism_pdk::Json<Value>) -> extism_pdk::FnResult<extism_pdk::Json<Value>> {
    Ok(extism_pdk::Json(
        process(input.0).map_err(extism_pdk::Error::msg)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint() -> Checkpoint {
        Checkpoint {
            inputs: vec![(13, false, 8)],
            ..Checkpoint::default()
        }
    }

    fn request(action: Option<&str>, state: &str, checkpoint: &Checkpoint) -> Value {
        json!({"version":2,"action":action,"state":state,"fields":{"checkpoint":serde_json::to_string(checkpoint).unwrap()},"locale":"en"})
    }

    fn grant(input: &mut Value, revision: i32, stored: &Records) {
        input["host"] = json!({"permissions":["storage"],"storage":{"revision":revision,"values":{STORAGE_KEY:stored}}});
    }

    fn saving(checkpoint: &Checkpoint, stored: &Records) -> Value {
        let mut input = request(Some("save-best"), "", checkpoint);
        grant(&mut input, 7, stored);
        process(input).unwrap()
    }

    fn success(output: &Value, checkpoint: &Checkpoint) -> Value {
        let mut input = request(
            Some("host-result"),
            output["state"].as_str().unwrap(),
            checkpoint,
        );
        input["results"] = json!([{"id":"save-best","value":{"revision":8,"values":output["effects"][0]["changes"]},"error":null}]);
        input
    }

    fn entry(score: u32, id: u32) -> ScoreEntry {
        ScoreEntry {
            score,
            kills: 1,
            foods: 2,
            ticks: 100,
            difficulty: Difficulty::Normal,
            run_id: format!("{id:08x}"),
        }
    }

    #[test]
    fn defaults_are_current_and_bilingual() {
        for (locale, display) in [("en", "Snake"), ("zh-CN", "贪吃蛇")] {
            let mut input = request(None, "", &checkpoint());
            input["locale"] = json!(locale);
            let result = process(input).unwrap();
            assert_eq!(result["display"], display);
            assert_eq!(result["effects"], json!([]));
            let state: State = serde_json::from_str(result["state"].as_str().unwrap()).unwrap();
            assert_eq!(state.schema, 3);
            assert_eq!(state.game.schema, 3);
            assert!(state.pending.is_none());
        }
    }

    #[test]
    fn non_axis_checkpoint_is_replayed_instead_of_trusting_summary_fields() {
        let checkpoint = checkpoint();
        let expected = summary(&replay(&checkpoint).unwrap());
        let result = process(request(Some("validate"), "", &checkpoint)).unwrap();
        assert_eq!(
            result["blocks"][0]["items"][0]["value"],
            expected.max_score.to_string()
        );
        assert_eq!(result["blocks"][0]["items"][2]["value"], "8");
        let mut forged = request(Some("save-best"), "", &checkpoint);
        let mut raw = serde_json::to_value(&checkpoint).unwrap();
        raw["maxScore"] = json!(999_999);
        forged["fields"]["checkpoint"] = json!(serde_json::to_string(&raw).unwrap());
        assert!(process(forged).is_err());
    }

    #[test]
    fn identity_is_stable_for_preferences_but_separates_seed_and_difficulty() {
        let original = checkpoint();
        assert_eq!(run_id(&original), "013528a21");
        let mut changed = original.clone();
        changed.skin = 7;
        changed.muted = true;
        changed.volume = 1;
        changed.nickname = "另外一个名字".into();
        changed.inputs = vec![(27, true, 1)];
        assert_eq!(run_id(&original), run_id(&changed));
        changed.seed += 1;
        assert_ne!(run_id(&original), run_id(&changed));
        changed.seed = original.seed;
        changed.difficulty = Difficulty::Hard;
        assert_ne!(run_id(&original), run_id(&changed));
    }

    #[test]
    fn exact_identity_separates_previously_colliding_runs() {
        for ((left_seed, left_mode), (right_seed, right_mode)) in [
            ((17_525, Difficulty::Normal), (81_704, Difficulty::Hard)),
            ((40_189, Difficulty::Normal), (797_186, Difficulty::Normal)),
        ] {
            let mut stored = Records::default();
            let mut ids = vec![];
            for (seed, difficulty) in [(left_seed, left_mode), (right_seed, right_mode)] {
                let checkpoint = Checkpoint {
                    seed,
                    difficulty,
                    ..checkpoint()
                };
                ids.push(run_id(&checkpoint));
                let result = summary(&replay(&checkpoint).unwrap());
                update_records(&mut stored, &checkpoint, &result);
                update_records(&mut stored, &checkpoint, &result);
            }
            assert_ne!(ids[0], ids[1]);
            assert_eq!(stored.scores.len(), 2);
            validate_records(&stored).unwrap();
        }
    }

    #[test]
    fn known_records_upgrade_in_memory_and_only_write_on_explicit_cas_save() {
        let checkpoint = checkpoint();
        let mut legacy = Records {
            schema: 3,
            ..Records::default()
        };
        legacy.best.insert("normal".into(), 1000);
        legacy.scores.push(entry(1000, 0x444d_0d79));
        let original = serde_json::to_value(&legacy).unwrap();
        for action in [None, Some("validate")] {
            let mut input = request(action, "", &checkpoint);
            grant(&mut input, 7, &legacy);
            let output = process(input.clone()).unwrap();
            assert_eq!(output["effects"], json!([]));
            assert_eq!(output["groups"][0]["actions"][1]["disabled"], false);
            assert_eq!(output["blocks"][2]["rows"][0][5], "444d0d79");
            assert_eq!(input["host"]["storage"]["values"][STORAGE_KEY], original);
        }
        let output = saving(&checkpoint, &legacy);
        let saved = &output["effects"][0]["changes"][STORAGE_KEY];
        assert_eq!(output["effects"][0]["expectedRevision"], 7);
        assert_eq!(saved["schema"], 4);
        assert_eq!(saved["scores"].as_array().unwrap().len(), 2);
        assert_eq!(saved["scores"][0]["runId"], "444d0d79");
        assert_eq!(saved["scores"][1]["runId"], run_id(&checkpoint));
        assert!(process(success(&output, &checkpoint)).is_ok());
    }

    #[test]
    fn record_identity_rejects_wrong_mode_zero_seed_and_malformed_upgrades() {
        let mut stored = Records::default();
        stored.best.insert("normal".into(), 1000);
        stored.scores.push(entry(1000, 1));
        for id in [
            "000000001",
            "000000012",
            "00000001f",
            "0000000g1",
            "0000000001",
        ] {
            stored.scores[0].run_id = id.into();
            assert!(validate_records(&stored).is_err(), "{id}");
        }
        stored.scores[0].run_id = "000000011".into();
        validate_records(&stored).unwrap();
        stored.schema = 3;
        assert!(records(Some(&serde_json::to_value(&stored).unwrap())).is_err());
        stored.scores[0].run_id = "00000001".into();
        assert_eq!(
            records(Some(&serde_json::to_value(&stored).unwrap()))
                .unwrap()
                .schema,
            4
        );
    }

    #[test]
    fn old_pending_states_and_old_schema_success_receipts_are_not_guessed() {
        let checkpoint = checkpoint();
        let output = saving(&checkpoint, &Records::default());
        let mut input = success(&output, &checkpoint);
        input["results"][0]["value"]["values"][STORAGE_KEY]["schema"] = json!(3);
        assert!(process(input).is_err());
        let mut input = success(&output, &checkpoint);
        let mut state: Value = serde_json::from_str(input["state"].as_str().unwrap()).unwrap();
        state["pending"]["expected"]["schema"] = json!(3);
        state["pending"]["expected"]["scores"][0]["runId"] = json!("444d0d79");
        input["state"] = json!(state.to_string());
        assert!(process(input).is_err());
    }

    #[test]
    fn malformed_actions_versions_locales_and_future_states_are_rejected() {
        let checkpoint = checkpoint();
        for key in ["version", "locale", "action", "state"] {
            let mut input = request(None, "", &checkpoint);
            input[key] = json!(false);
            assert!(process(input).is_err(), "{key}");
        }
        assert!(process(request(Some("unknown"), "", &checkpoint)).is_err());
        for schema in [1, 2, 4] {
            let state = json!({"schema":schema,"game":checkpoint,"notice":"ready","pending":null});
            assert!(process(request(None, &state.to_string(), &checkpoint)).is_err());
        }
    }

    #[test]
    fn checkpoint_contract_rejects_unknown_old_and_oversized_payloads() {
        let checkpoint = checkpoint();
        for mutation in [
            json!({"schema":2}),
            json!({"schema":4}),
            json!({"seed":0}),
            json!({"skin":15}),
            json!({"volume":101}),
            json!({"inputs":[[256,false,1]]}),
            json!({"inputs":[[0,false,0]]}),
            json!({"inputs":[[0,false,12001]]}),
            json!({"nickname":"1234567890123456789012345"}),
            json!({"theme":"neon"}),
        ] {
            let mut raw = serde_json::to_value(&checkpoint).unwrap();
            for (key, value) in mutation.as_object().unwrap() {
                raw[key] = value.clone();
            }
            let mut input = request(Some("validate"), "", &checkpoint);
            input["fields"]["checkpoint"] = json!(raw.to_string());
            assert!(process(input).is_err(), "{mutation}");
        }
        let mut input = request(Some("validate"), "", &checkpoint);
        input["fields"]["checkpoint"] = json!(" ".repeat(MAX_CHECKPOINT_BYTES + 1));
        assert!(process(input).is_err());
        assert!(process(request(None, &" ".repeat(MAX_STATE_BYTES + 1), &checkpoint)).is_err());
    }

    #[test]
    fn optional_storage_does_not_block_play_or_validation() {
        let checkpoint = checkpoint();
        for action in [None, Some("validate"), Some("save-best")] {
            let result = process(request(action, "", &checkpoint)).unwrap();
            assert_eq!(result["effects"], json!([]));
            assert_eq!(result["groups"][0]["actions"][1]["disabled"], true);
        }
    }

    #[test]
    fn legacy_future_and_damaged_records_are_preserved_read_only() {
        let checkpoint = checkpoint();
        for raw in [
            json!({"schema":1,"best":{},"scores":[]}),
            json!({"schema":2,"best":{},"scores":[]}),
            json!({"schema":5,"best":{},"scores":[]}),
            json!({"schema":3,"best":{},"scores":[{"score":1}]}),
        ] {
            let original = raw.clone();
            for action in [None, Some("validate"), Some("save-best")] {
                let mut input = request(action, "", &checkpoint);
                input["host"] = json!({"permissions":["storage"],"storage":{"revision":1,"values":{STORAGE_KEY:raw}}});
                let result = process(input.clone());
                if action == Some("save-best") {
                    assert!(result.is_err());
                } else {
                    let output = result.unwrap();
                    assert_eq!(output["effects"], json!([]));
                    assert_eq!(output["groups"][0]["actions"][1]["disabled"], true);
                    assert_eq!(output["blocks"][3]["kind"], "markdown");
                }
                assert_eq!(input["host"]["storage"]["values"][STORAGE_KEY], original);
            }
        }
    }

    #[test]
    fn invalid_storage_revisions_and_snapshots_disable_only_saving() {
        let checkpoint = checkpoint();
        for storage in [
            json!({"revision":-1,"values":{}}),
            json!({"revision":i32::MAX,"values":{}}),
            json!({"revision":0,"values":null}),
            json!({"revision":0.5,"values":{}}),
        ] {
            let mut input = request(Some("validate"), "", &checkpoint);
            input["host"] = json!({"permissions":["storage"],"storage":storage});
            let output = process(input).unwrap();
            assert_eq!(output["groups"][0]["actions"][1]["disabled"], true);
            assert_eq!(output["effects"], json!([]));
        }
    }

    #[test]
    fn save_uses_cas_and_keeps_other_difficulties() {
        let checkpoint = checkpoint();
        let mut stored = Records::default();
        stored.best.insert("hard".into(), 900);
        let output = saving(&checkpoint, &stored);
        assert_eq!(output["effects"][0]["expectedRevision"], 7);
        assert_eq!(
            output["effects"][0]["changes"][STORAGE_KEY]["best"]["hard"],
            900
        );
        let pending: State = serde_json::from_str(output["state"].as_str().unwrap()).unwrap();
        assert_eq!(pending.notice, "ready");
        assert!(pending.pending.is_some());
        assert_eq!(pending.game, checkpoint);
    }

    #[test]
    fn confirmed_receipt_wins_over_a_stale_snapshot() {
        let checkpoint = checkpoint();
        let output = saving(&checkpoint, &Records::default());
        let mut input = success(&output, &checkpoint);
        grant(&mut input, 7, &Records::default());
        let result = process(input).unwrap();
        assert_eq!(
            result["description"],
            "The best score for this difficulty was saved."
        );
        assert_eq!(
            result["blocks"][1]["rows"][0][1],
            summary(&replay(&checkpoint).unwrap()).max_score.to_string()
        );
        let state: State = serde_json::from_str(result["state"].as_str().unwrap()).unwrap();
        assert!(state.pending.is_none());
        assert_eq!(state.game, checkpoint);
    }

    #[test]
    fn receipts_require_the_entire_expected_leaderboard() {
        let checkpoint = checkpoint();
        let mut stored = Records::default();
        stored.best.insert("hard".into(), 900);
        let output = saving(&checkpoint, &stored);
        for property in ["best", "scores"] {
            let mut input = success(&output, &checkpoint);
            input["results"][0]["value"]["values"][STORAGE_KEY][property] = if property == "best" {
                json!({})
            } else {
                json!([])
            };
            assert!(process(input).is_err(), "{property}");
        }
        let mut input = success(&output, &checkpoint);
        input["results"][0]["value"]["values"][STORAGE_KEY]["best"]["hard"] = json!(901);
        assert!(process(input).is_err());
        let mut input = success(&output, &checkpoint);
        input["results"][0]["value"]["revision"] = json!(9);
        assert!(process(input).is_err());
    }

    #[test]
    fn receipt_identity_shape_and_pending_state_are_checked() {
        let checkpoint = checkpoint();
        let output = saving(&checkpoint, &Records::default());
        let state = output["state"].as_str().unwrap();
        assert!(process(request(None, state, &checkpoint)).is_err());
        assert!(process(request(Some("host-result"), "", &checkpoint)).is_err());
        for results in [
            json!([]),
            json!([{"id":"other","value":null,"error":{"code":"CONFLICT"}}]),
            json!([{"id":"save-best","value":{},"error":{"code":"CONFLICT"}}]),
            json!([{"id":"save-best","value":null,"error":{}}]),
            json!([{"id":"save-best","value":{"revision":8,"values":{}},"error":null}]),
        ] {
            let mut input = request(Some("host-result"), state, &checkpoint);
            input["results"] = results;
            assert!(process(input).is_err());
        }
    }

    #[test]
    fn conflict_cancel_omitted_and_failed_receipts_keep_the_game_without_retry() {
        let checkpoint = checkpoint();
        let output = saving(&checkpoint, &Records::default());
        for (code, notice) in [
            ("CONFLICT", "conflict"),
            ("USER_CANCELLED", "cancelled"),
            ("INPUT_TOO_LONG", "omitted"),
            ("STORAGE_IO", "failed"),
        ] {
            let mut input = request(
                Some("host-result"),
                output["state"].as_str().unwrap(),
                &checkpoint,
            );
            input["results"] = json!([{"id":"save-best","value":null,"error":{"code":code}}]);
            let result = process(input).unwrap();
            assert_eq!(result["effects"], json!([]));
            let state: State = serde_json::from_str(result["state"].as_str().unwrap()).unwrap();
            assert_eq!(state.game, checkpoint);
            assert_eq!(state.notice, notice);
            assert!(state.pending.is_none());
        }
    }

    #[test]
    fn peak_score_is_saved_even_when_boost_consumed_length() {
        let checkpoint = Checkpoint {
            inputs: vec![(13, false, 8), (81, false, 15), (130, true, 12)],
            ..Checkpoint::default()
        };
        let result = summary(&replay(&checkpoint).unwrap());
        // The nearby three-mass pellet grows the snake from 10 to 13;
        // boosting subsequently spends that mass, while retaining its peak.
        assert_eq!(result.score, 10);
        assert_eq!(result.max_score, 13);
        assert_eq!(result.foods, 1);
        assert_eq!(result.ticks, 35);
        let output = saving(&checkpoint, &Records::default());
        let saved = &output["effects"][0]["changes"][STORAGE_KEY];
        assert_eq!(saved["best"]["normal"], 13);
        assert_eq!(saved["scores"][0]["score"], 13);
        assert_eq!(saved["scores"][0]["ticks"], 35);
    }

    #[test]
    fn duplicate_run_saves_never_lower_or_multiply_a_record() {
        let checkpoint = checkpoint();
        let mut result = summary(&replay(&checkpoint).unwrap());
        let mut stored = Records::default();
        for score in [200, 400, 400, 100] {
            result.max_score = score;
            update_records(&mut stored, &checkpoint, &result);
        }
        assert_eq!(stored.scores.len(), 1);
        assert_eq!(stored.scores[0].score, 400);
        assert_eq!(stored.best["normal"], 400);
    }

    #[test]
    fn leaderboard_is_top_ten_and_ties_remain_stable() {
        let checkpoint = checkpoint();
        let mut stored = Records::default();
        stored.best.insert("normal".into(), 1000);
        stored.scores = (0..10).map(|id| entry(100, id)).collect();
        let mut result = summary(&replay(&checkpoint).unwrap());
        result.max_score = 200;
        update_records(&mut stored, &checkpoint, &result);
        assert_eq!(stored.scores.len(), 10);
        assert_eq!(stored.scores[0].run_id, run_id(&checkpoint));
        assert_eq!(stored.scores[1].run_id, "00000000");
        assert_eq!(stored.scores[9].run_id, "00000008");
        validate_records(&stored).unwrap();
    }

    #[test]
    fn record_boundary_rejects_duplicates_order_unknown_difficulty_and_counters() {
        let mut valid = Records::default();
        valid.best.insert("normal".into(), 1000);
        valid.scores = vec![entry(300, 1), entry(200, 2)];
        validate_records(&valid).unwrap();
        for mutation in [
            json!({"schema":5}),
            json!({"best":{"unknown":1000}}),
            json!({"best":{"normal":1}}),
            json!({"scores":[entry(300,1),entry(200,1)]}),
            json!({"scores":[entry(200,1),entry(300,2)]}),
        ] {
            let mut raw = serde_json::to_value(&valid).unwrap();
            for (key, value) in mutation.as_object().unwrap() {
                raw[key] = value.clone();
            }
            assert!(records(Some(&raw)).is_err(), "{mutation}");
        }
        for (field, value) in [
            ("score", 0),
            ("score", MAX_RECORD_SCORE + 1),
            ("ticks", 0),
            ("ticks", MAX_TICKS + 1),
            ("kills", MAX_RECORD_KILLS + 1),
            ("foods", MAX_RECORD_FOODS + 1),
        ] {
            let mut raw = serde_json::to_value(&valid).unwrap();
            raw["scores"][0][field] = json!(value);
            assert!(records(Some(&raw)).is_err(), "{field}={value}");
        }
    }

    #[test]
    fn menu_checkpoint_does_not_create_a_ranked_run() {
        let checkpoint = Checkpoint::default();
        let output = saving(&checkpoint, &Records::default());
        assert_eq!(
            output["effects"][0]["changes"][STORAGE_KEY]["scores"],
            json!([])
        );
        assert!(process(success(&output, &checkpoint)).is_ok());
    }

    #[test]
    fn tampered_pending_expected_record_is_rejected() {
        let checkpoint = checkpoint();
        let output = saving(&checkpoint, &Records::default());
        let mut input = success(&output, &checkpoint);
        let mut state: Value = serde_json::from_str(input["state"].as_str().unwrap()).unwrap();
        state["pending"]["expected"]["scores"] = json!([]);
        input["state"] = json!(state.to_string());
        assert!(process(input).is_err());
    }
}
