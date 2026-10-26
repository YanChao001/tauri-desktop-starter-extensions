//! Snake rules are replayed from a bounded log before a record may be persisted.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const SIZE: i16 = 21;
const MAX_MOVES: usize = 8192;
const MAX_SCORE: u32 = (SIZE as u32 * SIZE as u32 - 3) * 10;
const DEFAULT_SEED: u32 = 20261026;
const STORAGE_KEY: &str = "snake-records";
type Point = (i16, i16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Up,
    Right,
    Down,
    Left,
}
impl Direction {
    fn opposite(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Up, Self::Down)
                | (Self::Down, Self::Up)
                | (Self::Left, Self::Right)
                | (Self::Right, Self::Left)
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Difficulty {
    Slow,
    Normal,
    Fast,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    schema: u8,
    seed: u32,
    difficulty: Difficulty,
    wrap: bool,
    obstacles: bool,
    moves: Vec<Direction>,
    muted: bool,
}
impl Default for Checkpoint {
    fn default() -> Self {
        Self {
            schema: 1,
            seed: DEFAULT_SEED,
            difficulty: Difficulty::Normal,
            wrap: false,
            obstacles: false,
            moves: vec![],
            muted: false,
        }
    }
}
impl Checkpoint {
    fn mode(&self) -> String {
        format!(
            "{}-{}-{}",
            match self.difficulty {
                Difficulty::Slow => "slow",
                Difficulty::Normal => "normal",
                Difficulty::Fast => "fast",
            },
            if self.wrap { "wrap" } else { "wall" },
            if self.obstacles { "rocks" } else { "clear" }
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Game {
    snake: Vec<Point>,
    food: Option<Point>,
    direction: Direction,
    rng: u32,
    score: u32,
    dead: bool,
    wrap: bool,
    rocks: Vec<Point>,
}
impl Game {
    fn new(checkpoint: &Checkpoint) -> Self {
        let mut game = Self {
            snake: vec![(10, 10), (9, 10), (8, 10)],
            food: None,
            direction: Direction::Right,
            rng: checkpoint.seed,
            score: 0,
            dead: false,
            wrap: checkpoint.wrap,
            rocks: if checkpoint.obstacles {
                vec![(5, 5), (15, 5), (5, 15), (15, 15), (10, 5), (10, 15)]
            } else {
                vec![]
            },
        };
        game.spawn();
        game
    }
    fn random(&mut self) -> u32 {
        let mut value = self.rng;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.rng = value;
        value
    }
    fn spawn(&mut self) {
        let empty: Vec<_> = (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .filter(|point| !self.snake.contains(point) && !self.rocks.contains(point))
            .collect();
        self.food = if empty.is_empty() {
            None
        } else {
            Some(empty[self.random() as usize % empty.len()])
        };
    }
    fn step(&mut self, direction: Direction) -> Result<(), String> {
        if self.dead || self.food.is_none() || self.direction.opposite(direction) {
            return Err("Unreachable snake history / 贪吃蛇操作历史不可达".into());
        }
        self.direction = direction;
        let (x, y) = self.snake[0];
        let (dx, dy) = match direction {
            Direction::Up => (0, -1),
            Direction::Right => (1, 0),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
        };
        let mut head = (x + dx, y + dy);
        if self.wrap {
            head = (head.0.rem_euclid(SIZE), head.1.rem_euclid(SIZE));
        }
        let eating = self.food == Some(head);
        let occupied = &self.snake[..self.snake.len() - usize::from(!eating)];
        if !(0..SIZE).contains(&head.0)
            || !(0..SIZE).contains(&head.1)
            || self.rocks.contains(&head)
            || occupied.contains(&head)
        {
            self.dead = true;
            return Ok(());
        }
        self.snake.insert(0, head);
        if eating {
            self.score += 10;
            self.spawn();
        } else {
            self.snake.pop();
        }
        Ok(())
    }
}
fn replay(checkpoint: &Checkpoint) -> Result<Game, String> {
    if checkpoint.schema != 1 || checkpoint.seed == 0 || checkpoint.moves.len() > MAX_MOVES {
        return Err("Invalid snake checkpoint / 无效的贪吃蛇进度".into());
    }
    let mut game = Game::new(checkpoint);
    for direction in &checkpoint.moves {
        game.step(*direction)?;
    }
    Ok(game)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Records {
    schema: u8,
    best: BTreeMap<String, u32>,
}
impl Default for Records {
    fn default() -> Self {
        Self {
            schema: 1,
            best: BTreeMap::new(),
        }
    }
}
fn records(raw: Option<&Value>) -> Result<Records, String> {
    let Some(raw) = raw else {
        return Ok(Records::default());
    };
    let result: Records = serde_json::from_value(raw.clone())
        .map_err(|_| "Invalid or future snake records / 纪录损坏或版本较新，已保留原件")?;
    if result.schema != 1
        || result.best.len() > 12
        || result.best.iter().any(|(key, score)| {
            let parts: Vec<_> = key.split('-').collect();
            parts.len() != 3
                || !["slow", "normal", "fast"].contains(&parts[0])
                || !["wall", "wrap"].contains(&parts[1])
                || !["clear", "rocks"].contains(&parts[2])
                || *score > MAX_SCORE
                || !score.is_multiple_of(10)
        })
    {
        return Err("Invalid or future snake records / 纪录损坏或版本较新，已保留原件".into());
    }
    Ok(result)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    revision: i32,
    mode: String,
    best: u32,
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
            schema: 1,
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
    if raw.len() > 128_000 {
        return Err("Snake state too large".into());
    }
    let mut state: State = if raw.is_empty() {
        State::default()
    } else {
        serde_json::from_str(raw).map_err(|_| "Invalid snake state")?
    };
    if state.schema != 1
        || ![
            "ready",
            "checked",
            "saved",
            "conflict",
            "failed",
            "omitted",
            "no-storage",
        ]
        .contains(&state.notice.as_str())
    {
        return Err("Invalid snake state".into());
    }
    let action = request["action"].as_str().unwrap_or("");
    if state.pending.is_some() != (action == "host-result") {
        return Err("Unexpected or missing storage receipt".into());
    }
    if matches!(action, "validate" | "save-best") {
        let checkpoint = request["fields"]["checkpoint"]
            .as_str()
            .ok_or("Missing snake checkpoint")?;
        if checkpoint.len() > 120_000 {
            return Err("Snake checkpoint too large".into());
        }
        state.game = serde_json::from_str(checkpoint).map_err(|_| "Invalid snake checkpoint")?;
    }
    let game = replay(&state.game)?;
    let granted = request["host"]["permissions"]
        .as_array()
        .is_some_and(|list| list.iter().any(|value| value == "storage"));
    let snapshot = &request["host"]["storage"];
    let (revision, mut stored) = if granted {
        let revision = snapshot["revision"]
            .as_i64()
            .filter(|revision| (0..i32::MAX as i64).contains(revision))
            .ok_or("Invalid storage revision")? as i32;
        let values = snapshot["values"]
            .as_object()
            .ok_or("Invalid storage values")?;
        (revision, records(values.get(STORAGE_KEY))?)
    } else {
        (0, Records::default())
    };
    let mode = state.game.mode();
    let mut effects = vec![];
    match action {
        "" => {}
        "validate" => state.notice = "checked".into(),
        "save-best" if !granted => state.notice = "no-storage".into(),
        "save-best" => {
            let best = stored.best.get(&mode).copied().unwrap_or(0).max(game.score);
            stored.best.insert(mode.clone(), best);
            state.pending = Some(Pending {
                revision,
                mode: mode.clone(),
                best,
            });
            effects.push(json!({"kind":"storage","id":"save-best","expectedRevision":revision,"changes":{STORAGE_KEY:stored}}));
        }
        "host-result" => {
            let pending = state.pending.take().ok_or("Missing pending result")?;
            if pending.revision < 0
                || pending.revision == i32::MAX
                || pending.mode != mode
                || pending.best < game.score
                || pending.best > MAX_SCORE
                || !pending.best.is_multiple_of(10)
            {
                return Err("Invalid pending snake record".into());
            }
            let results = request["results"]
                .as_array()
                .ok_or("Missing effect results")?;
            if results.len() != 1 || results[0]["id"] != "save-best" {
                return Err("Invalid snake storage receipt".into());
            }
            state.notice = if results[0]["error"].is_null() {
                let value = &results[0]["value"];
                let saved = records(value["values"].as_object().and_then(|v| v.get(STORAGE_KEY)))?;
                if value["revision"].as_i64() != Some(i64::from(pending.revision) + 1)
                    || saved.best.get(&mode).copied().unwrap_or(0) < pending.best
                {
                    return Err("Invalid snake success receipt".into());
                }
                "saved"
            } else {
                match results[0]["error"]["code"].as_str() {
                    Some("CONFLICT") => "conflict",
                    Some("INPUT_TOO_LONG") => "omitted",
                    Some(_) => "failed",
                    None => return Err("Invalid snake failure receipt".into()),
                }
            }
            .into();
        }
        _ => return Err("Unknown snake action / 未知的贪吃蛇操作".into()),
    }
    let description = match state.notice.as_str() {
        "checked" => tr(
            zh,
            "进度已由 Wasm 重放核验。",
            "The checkpoint was replayed and validated by Wasm.",
        ),
        "saved" => tr(
            zh,
            "当前模式纪录已保存。",
            "The best score for this mode was saved.",
        ),
        "conflict" => tr(
            zh,
            "另一窗口更新了纪录；棋局保留，请审阅后手动再次保存。",
            "Another window changed records. Your game is retained; review and save again manually.",
        ),
        "omitted" => tr(
            zh,
            "保存操作已执行，响应被省略；棋局保留，不要自动重试。",
            "The save was executed but its response was omitted. Your game is retained; do not automatically retry.",
        ),
        "failed" => tr(
            zh,
            "保存未完成；棋局保留，未自动重试。",
            "The save did not complete. Your game is retained; no automatic retry was made.",
        ),
        "no-storage" => tr(
            zh,
            "未授权私有存储，仍可完整游玩和导出会话。",
            "Private storage is not granted. Play and session export remain available.",
        ),
        _ => tr(
            zh,
            "方向键 / WASD / 滑动控制；空格暂停。离开后自动暂停，返回时手动继续。",
            "Use arrows / WASD / swipe; Space pauses. Leaving pauses the game; resume manually when you return.",
        ),
    };
    let best = stored.best.get(&mode).copied().unwrap_or(0);
    let record_rows: Vec<_> = stored
        .best
        .iter()
        .map(|(mode, best)| vec![mode.clone(), best.to_string()])
        .collect();
    Ok(json!({
        "version":2,"display":tr(zh,"贪吃蛇","Snake"),"description":description,
        "state":serde_json::to_string(&state).map_err(|_| "Could not encode snake state")?,
        "fields":[{"id":"checkpoint","label":tr(zh,"棋局进度","Game checkpoint"),"value":serde_json::to_string(&state.game).map_err(|_| "Could not encode checkpoint")?,"kind":"textarea","maxBytes":120000}],
        "groups":[{"columns":2,"actions":[
            {"id":"validate","label":tr(zh,"核验进度","Validate checkpoint"),"disabled":false,"primary":false},
            {"id":"save-best","label":tr(zh,"保存纪录","Save best score"),"disabled":!granted,"primary":true}
        ]}],
        "blocks":[{"kind":"metrics","items":[
            {"label":tr(zh,"分数","Score"),"value":game.score.to_string()},
            {"label":tr(zh,"模式纪录","Mode best"),"value":best.to_string()},
            {"label":tr(zh,"步数","Ticks"),"value":state.game.moves.len().to_string()}
        ]},{"kind":"table","columns":["mode","best"],"rows":record_rows}],"effects":effects,"artifacts":[],"errors":{}
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
    fn request(action: Option<&str>, state: &str, checkpoint: &Checkpoint) -> Value {
        json!({"version":2,"action":action,"state":state,"fields":{"checkpoint":serde_json::to_string(checkpoint).unwrap()},"locale":"en"})
    }
    #[test]
    fn initial_food_and_replay_are_deterministic() {
        let checkpoint = Checkpoint::default();
        let first = replay(&checkpoint).unwrap();
        assert_eq!(first, replay(&checkpoint).unwrap());
        assert!(!first.snake.contains(&first.food.unwrap()));
        assert_eq!(first.food, Some((4, 2)));
    }
    #[test]
    fn walls_kill_but_wrap_continues_and_reversal_is_rejected() {
        let mut checkpoint = Checkpoint {
            moves: vec![Direction::Right; 11],
            ..Checkpoint::default()
        };
        assert!(replay(&checkpoint).unwrap().dead);
        checkpoint.moves.push(Direction::Down);
        assert!(replay(&checkpoint).is_err());
        checkpoint.wrap = true;
        assert!(!replay(&checkpoint).unwrap().dead);
        checkpoint.moves = vec![Direction::Left];
        assert!(replay(&checkpoint).is_err());
    }
    #[test]
    fn food_growth_obstacles_and_tail_vacating_rules() {
        let mut game = Game::new(&Checkpoint::default());
        game.food = Some((11, 10));
        game.step(Direction::Right).unwrap();
        assert_eq!(game.score, 10);
        assert_eq!(game.snake.len(), 4);
        game.snake = vec![(10, 10), (10, 11), (9, 11), (9, 10)];
        game.direction = Direction::Up;
        game.food = Some((0, 0));
        game.step(Direction::Left).unwrap();
        assert!(!game.dead);
        game.rocks.push((8, 10));
        game.step(Direction::Left).unwrap();
        assert!(game.dead);
    }
    #[test]
    fn invalid_imports_and_unknown_actions_are_rejected() {
        let checkpoint = Checkpoint::default();
        let mut value = serde_json::to_value(&checkpoint).unwrap();
        value["score"] = json!(999);
        assert!(serde_json::from_value::<Checkpoint>(value).is_err());
        let mut invalid = checkpoint.clone();
        invalid.schema = 2;
        assert!(replay(&invalid).is_err());
        invalid = checkpoint.clone();
        invalid.seed = 0;
        assert!(replay(&invalid).is_err());
        invalid = checkpoint.clone();
        invalid.moves = vec![Direction::Right; MAX_MOVES + 1];
        assert!(replay(&invalid).is_err());
        assert!(process(request(Some("unknown"), "", &checkpoint)).is_err());
    }
    #[test]
    fn storage_is_optional_and_future_records_are_preserved() {
        let checkpoint = Checkpoint::default();
        let result = process(request(Some("save-best"), "", &checkpoint)).unwrap();
        assert!(result["effects"].as_array().unwrap().is_empty());
        let mut invalid = request(Some("save-best"), "", &checkpoint);
        invalid["host"] = json!({"permissions":["storage"],"storage":{"revision":1,"values":{STORAGE_KEY:{"schema":2,"best":{}}}}});
        assert!(process(invalid).is_err());
        assert!(records(Some(&json!({"schema":1,"best":{"other":10}}))).is_err());
    }
    #[test]
    fn saving_checks_revision_receipts_and_keeps_other_modes() {
        let checkpoint = Checkpoint::default();
        let mut input = request(Some("save-best"), "", &checkpoint);
        input["host"] = json!({"permissions":["storage"],"storage":{"revision":7,"values":{STORAGE_KEY:{"schema":1,"best":{"fast-wrap-rocks":50}}}}});
        let output = process(input).unwrap();
        assert_eq!(output["effects"][0]["expectedRevision"], 7);
        assert_eq!(
            output["effects"][0]["changes"][STORAGE_KEY]["best"]["fast-wrap-rocks"],
            50
        );
        let state = output["state"].as_str().unwrap();
        let mut receipt = request(Some("host-result"), state, &checkpoint);
        receipt["results"] = json!([{"id":"save-best","value":{"revision":8,"values":{STORAGE_KEY:{"schema":1,"best":{"normal-wall-clear":0,"fast-wrap-rocks":50}}}},"error":null}]);
        assert!(
            process(receipt.clone()).unwrap()["description"]
                .as_str()
                .unwrap()
                .contains("was saved")
        );
        receipt["results"][0]["value"]["revision"] = json!(9);
        assert!(process(receipt.clone()).is_err());
        receipt["results"][0] = json!({"id":"save-best","value":null,"error":{"code":"CONFLICT","message":"changed","retryable":false}});
        let failed = process(receipt).unwrap();
        assert!(failed["effects"].as_array().unwrap().is_empty());
        let retained: State = serde_json::from_str(failed["state"].as_str().unwrap()).unwrap();
        assert_eq!(retained.game, checkpoint);
        assert!(retained.pending.is_none());
    }
}
