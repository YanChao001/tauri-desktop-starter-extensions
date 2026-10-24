//! Deterministic 2048: all boards and scores are replayed from a bounded move log.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MAX_MOVES: usize = 4096;
const DEFAULT_SEED: u32 = 20261025;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Up,
    Left,
    Down,
    Right,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Notice {
    #[default]
    Ready,
    Moved,
    NoMove,
    Undone,
    NewGame,
    Saved,
    SaveFailed,
    SaveConflict,
    NoStorage,
    Limit,
    Unknown,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u8,
    seed: u32,
    moves: Vec<Direction>,
    notice: Notice,
    pending: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: 1,
            seed: DEFAULT_SEED,
            moves: vec![],
            notice: Notice::Ready,
            pending: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Game {
    board: [u16; 16],
    score: u64,
    rng: u32,
}
impl Game {
    fn new(seed: u32) -> Self {
        let mut game = Self {
            board: [0; 16],
            score: 0,
            rng: seed,
        };
        game.spawn();
        game.spawn();
        game
    }
    fn random(&mut self) -> u32 {
        let mut n = self.rng;
        n ^= n << 13;
        n ^= n >> 17;
        n ^= n << 5;
        self.rng = n;
        n
    }
    fn spawn(&mut self) {
        let empty: Vec<_> = self
            .board
            .iter()
            .enumerate()
            .filter(|(_, n)| **n == 0)
            .map(|(i, _)| i)
            .collect();
        if empty.is_empty() {
            return;
        }
        let index = empty[self.random() as usize % empty.len()];
        self.board[index] = if self.random().is_multiple_of(10) {
            4
        } else {
            2
        };
    }
    fn slide(&mut self, direction: Direction) -> bool {
        let before = self.board;
        for row in 0..4 {
            let indices: [usize; 4] = std::array::from_fn(|col| match direction {
                Direction::Left => row * 4 + col,
                Direction::Right => row * 4 + 3 - col,
                Direction::Up => col * 4 + row,
                Direction::Down => (3 - col) * 4 + row,
            });
            let values: Vec<_> = indices
                .iter()
                .map(|i| self.board[*i])
                .filter(|n| *n > 0)
                .collect();
            let mut merged = [0; 4];
            let mut cursor = 0;
            let mut output = 0;
            while cursor < values.len() {
                if cursor + 1 < values.len() && values[cursor] == values[cursor + 1] {
                    // A reachable board has total mass <= 16_392, so u16 cannot overflow.
                    merged[output] = values[cursor] * 2;
                    self.score += u64::from(merged[output]);
                    cursor += 2;
                } else {
                    merged[output] = values[cursor];
                    cursor += 1;
                }
                output += 1;
            }
            for (index, value) in indices.iter().zip(merged) {
                self.board[*index] = value;
            }
        }
        before != self.board
    }
    fn step(&mut self, direction: Direction) -> bool {
        if !self.slide(direction) {
            return false;
        }
        self.spawn();
        true
    }
    fn legal(&self, direction: Direction) -> bool {
        let mut copy = self.clone();
        copy.slide(direction)
    }
    fn lost(&self) -> bool {
        [
            Direction::Up,
            Direction::Left,
            Direction::Down,
            Direction::Right,
        ]
        .iter()
        .all(|d| !self.legal(*d))
    }
}

fn replay(state: &State) -> Result<Game, String> {
    if state.schema != 1
        || state.seed == 0
        || state.seed > 999_999_999
        || state.moves.len() > MAX_MOVES
    {
        return Err("Invalid 2048 state / 无效的 2048 状态".into());
    }
    let mut game = Game::new(state.seed);
    for direction in &state.moves {
        if !game.step(*direction) {
            return Err("Unreachable move history / 操作历史不可达".into());
        }
    }
    Ok(game)
}
fn tr<'a>(zh: bool, cn: &'a str, en: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn action(id: &str, label: &str, disabled: bool, primary: bool) -> Value {
    json!({"id":id,"label":label,"disabled":disabled,"primary":primary})
}
fn storage(request: &Value) -> Option<&Value> {
    let host = &request["host"];
    if !host["permissions"]
        .as_array()?
        .iter()
        .any(|p| p == "storage")
    {
        return None;
    }
    let snapshot = &host["storage"];
    snapshot["revision"]
        .as_i64()
        .filter(|r| (0..=i32::MAX as i64).contains(r))?;
    snapshot["values"].as_object()?;
    Some(snapshot)
}

pub fn process(request: Value) -> Result<Value, String> {
    if request["version"] != 2 || !matches!(request["locale"].as_str(), Some("zh-CN" | "en")) {
        return Err("Unsupported view request / 不支持的视图请求".into());
    }
    let zh = request["locale"] == "zh-CN";
    let raw = request["state"]
        .as_str()
        .ok_or("Missing state / 缺少状态")?;
    if raw.len() > 65_536 {
        return Err("2048 state too large / 状态过大".into());
    }
    let mut state: State = if raw.is_empty() {
        State::default()
    } else {
        serde_json::from_str(raw).map_err(|_| "Invalid 2048 state / 无效状态")?
    };
    let mut game = replay(&state)?;
    let current_action = request["action"].as_str().unwrap_or("");
    if state.pending && current_action != "host-result" {
        return Err("Missing storage receipt / 缺少存储回执".into());
    }
    let snapshot = storage(&request);
    let stored_best = snapshot
        .and_then(|s| s["values"]["bestScore"].as_u64())
        .filter(|n| *n <= 1_000_000)
        .unwrap_or(0);
    let mut effects = vec![];
    let mut errors = json!({});
    let seed_field = request["fields"]["seed"].as_str().unwrap_or("");
    let mut displayed_seed = state.seed.to_string();
    match current_action {
        "" => {}
        "up" | "left" | "down" | "right" => {
            let direction = match current_action {
                "up" => Direction::Up,
                "left" => Direction::Left,
                "down" => Direction::Down,
                _ => Direction::Right,
            };
            if state.moves.len() == MAX_MOVES {
                state.notice = Notice::Limit;
            } else if game.step(direction) {
                state.moves.push(direction);
                state.notice = Notice::Moved;
            } else {
                state.notice = Notice::NoMove;
            }
        }
        "undo" => {
            if state.moves.pop().is_some() {
                game = replay(&state)?;
                state.notice = Notice::Undone;
            } else {
                state.notice = Notice::NoMove;
            }
        }
        "new-game" => {
            let seed = if seed_field.is_empty() {
                Ok(state.seed)
            } else {
                seed_field.parse::<u32>()
            };
            if let Ok(seed @ 1..=999_999_999) = seed {
                state = State {
                    seed,
                    notice: Notice::NewGame,
                    ..State::default()
                };
                game = replay(&state)?;
                displayed_seed = seed.to_string();
            } else {
                errors["seed"] = json!(tr(
                    zh,
                    "请输入 1–999999999 的整数种子。",
                    "Enter an integer seed from 1 to 999999999."
                ));
                displayed_seed = state.seed.to_string();
            }
        }
        "save-best" => {
            if let Some(snapshot) = snapshot {
                state.pending = true;
                effects.push(json!({"kind":"storage","id":"save-best","expectedRevision":snapshot["revision"],"changes":{"bestScore":stored_best.max(game.score)}}));
            } else {
                state.notice = Notice::NoStorage;
            }
        }
        "host-result" => {
            if !state.pending {
                return Err("Unexpected storage receipt / 非预期存储回执".into());
            }
            let results = request["results"]
                .as_array()
                .ok_or("Missing storage result")?;
            if results.len() != 1 || results[0]["id"] != "save-best" {
                return Err("Invalid storage receipt / 无效存储回执".into());
            }
            state.pending = false;
            state.notice = if results[0]["error"].is_null() {
                Notice::Saved
            } else if results[0]["error"]["code"] == "CONFLICT" {
                Notice::SaveConflict
            } else {
                Notice::SaveFailed
            };
        }
        _ => state.notice = Notice::Unknown,
    }
    let best = stored_best.max(game.score);
    let won = game.board.iter().any(|n| *n >= 2048);
    let lost = game.lost();
    let show_board_status = matches!(
        state.notice,
        Notice::Ready | Notice::Moved | Notice::NoMove | Notice::NewGame
    );
    let status = if lost && show_board_status {
        tr(
            zh,
            "棋盘已满且没有可合并的方块，试试撤销或重新开局。",
            "No moves remain. Undo a move or start a new game.",
        )
    } else if won && show_board_status {
        tr(
            zh,
            "已合成 2048！你可以继续挑战更大的方块。",
            "2048 reached! Keep playing for a larger tile.",
        )
    } else {
        match state.notice {
            Notice::Ready | Notice::NewGame => tr(
                zh,
                "合并相同数字，向 2048 出发。相同种子产生相同开局。",
                "Merge equal tiles toward 2048. The same seed reproduces the same game.",
            ),
            Notice::Moved => tr(
                zh,
                "移动成功：有效移动后随机生成一个 2 或 4。",
                "Moved: a new 2 or 4 appears after each valid move.",
            ),
            Notice::NoMove => tr(
                zh,
                "这个方向不会改变棋盘，没有消耗回合。",
                "That move would not change the board; no turn was used.",
            ),
            Notice::Undone => tr(
                zh,
                "已撤销一步，棋盘、分数和随机序列一并恢复。",
                "Undone: board, score and random sequence restored.",
            ),
            Notice::Saved => tr(
                zh,
                "纪录已保存到此插件的私有存储。",
                "Best score saved to this plugin's private storage.",
            ),
            Notice::SaveFailed => tr(
                zh,
                "保存失败；未自动重试。请检查存储后手动再保存。",
                "Save failed; no automatic retry. Check storage and save again manually.",
            ),
            Notice::SaveConflict => tr(
                zh,
                "另一会话更新了纪录。请核对后手动再次保存。",
                "Another session changed storage. Review it and save again manually.",
            ),
            Notice::NoStorage => tr(
                zh,
                "未授权私有存储；本局仍可正常游玩。",
                "Private storage is not granted; this game remains playable.",
            ),
            Notice::Limit => tr(
                zh,
                "本局达到 4096 步上限，可撤销或使用新种子重新开局。",
                "The 4096-move session limit was reached. Undo or start a new seed.",
            ),
            Notice::Unknown => tr(
                zh,
                "不支持此操作，棋盘保持原样。",
                "Unsupported action; the board was kept.",
            ),
        }
    };
    let cells: Vec<_> = game.board.iter().enumerate().map(|(i, n)| {
        let label = if *n == 0 { "·".into() } else { n.to_string() };
        json!({"id":format!("tile-{i}"),"label":label,"disabled":true,"primary":*n>=128,"ariaLabel":format!("{} {}, {} {}, {}",tr(zh,"行","Row"),i/4+1,tr(zh,"列","column"),i%4+1,if *n==0 {tr(zh,"空","empty").into()} else {n.to_string()})})
    }).collect();
    let controls = vec![
        action(
            "up",
            tr(zh, "↑ 上", "↑ Up"),
            !game.legal(Direction::Up),
            true,
        ),
        action(
            "left",
            tr(zh, "← 左", "← Left"),
            !game.legal(Direction::Left),
            true,
        ),
        action(
            "down",
            tr(zh, "↓ 下", "↓ Down"),
            !game.legal(Direction::Down),
            true,
        ),
        action(
            "right",
            tr(zh, "→ 右", "→ Right"),
            !game.legal(Direction::Right),
            true,
        ),
    ];
    Ok(
        json!({"version":2,"state":serde_json::to_string(&state).map_err(|e|e.to_string())?,"display":"2048","description":status,
        "fields":[{"id":"seed","label":tr(zh,"新开局种子（整数）","Seed for new game (integer)"),"value":displayed_seed,"kind":"number","maxBytes":9}],
        "groups":[{"columns":4,"board":true,"actions":cells},{"columns":4,"actions":controls},{"columns":3,"actions":[action("undo",tr(zh,"↶ 撤销一步","↶ Undo"),state.moves.is_empty(),false),action("new-game",tr(zh,"重新开局","New game"),false,false),action("save-best",tr(zh,"保存当前纪录","Save best score"),snapshot.is_none(),false)]}],
        "blocks":[{"kind":"metrics","items":[{"label":tr(zh,"分数","Score"),"value":game.score.to_string()},{"label":tr(zh,"纪录（本局 / 已保存）","Best (current / saved)"),"value":best.to_string()},{"label":tr(zh,"步数","Moves"),"value":state.moves.len().to_string()}]},
        {"kind":"markdown","text":tr(zh,"**规则**：点击方向按钮将方块推到边缘。相邻相同数字每步只合并一次，合并得到的数字计入分数。无效移动不产生新方块。\n\n更换种子后点击“重新开局”可换题；同种子与同操作历史得到相同结果。撤销只影响本局，已保存纪录不会回退。可选存储只保存最高分，**不自动保存棋盘**。离开前可用宿主导出会话保留本局。","**Rules:** Use direction buttons to slide tiles to an edge. Equal adjacent tiles merge once per move; the resulting number adds to your score. Invalid moves spawn nothing.\n\nChange the seed and select New game for a different board. The same seed and moves reproduce the same result. Undo affects this game; a saved best score is retained. Optional storage saves only the best score, **not the board**. Use the host's session export to keep a game.")}],"errors":errors,"effects":effects}),
    )
}

#[cfg(target_arch = "wasm32")]
#[extism_pdk::plugin_fn]
pub fn play(input: String) -> extism_pdk::FnResult<String> {
    let request: Value = serde_json::from_str(&input)?;
    let view = process(request).map_err(extism_pdk::Error::msg)?;
    Ok(serde_json::to_string(&view)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(state: &State, action: &str) -> Value {
        json!({"version":2,"locale":"en","state":serde_json::to_string(state).unwrap(),"fields":{},"action":action,"host":{"permissions":[],"storage":null}})
    }
    #[test]
    fn merges_only_once_and_counts_resulting_tiles() {
        let mut game = Game {
            board: [2, 2, 2, 2, 4, 0, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0],
            score: 0,
            rng: 1,
        };
        assert!(game.slide(Direction::Left));
        assert_eq!(&game.board[..8], &[4, 4, 0, 0, 8, 4, 0, 0]);
        assert_eq!(game.score, 16);
    }
    #[test]
    fn directions_preserve_mass_and_invalid_moves_do_not_advance_rng() {
        for direction in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let mut game = Game::new(42);
            let mass: u16 = game.board.iter().sum();
            game.slide(direction);
            assert_eq!(game.board.iter().sum::<u16>(), mass);
        }
        let mut game = Game {
            board: [
                2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2, 4, 8, 16, 32, 64,
            ],
            score: 0,
            rng: 42,
        };
        let before = game.clone();
        assert!(game.lost());
        assert!(!game.step(Direction::Left));
        assert_eq!(game, before);
    }
    #[test]
    fn undo_restores_rng_score_and_board_exactly() {
        let mut state = State::default();
        let first = replay(&state).unwrap();
        let direction = [
            Direction::Left,
            Direction::Up,
            Direction::Right,
            Direction::Down,
        ]
        .into_iter()
        .find(|d| first.legal(*d))
        .unwrap();
        state.moves.push(direction);
        let second = replay(&state).unwrap();
        state.moves.pop();
        assert_eq!(replay(&state).unwrap(), first);
        state.moves.push(direction);
        assert_eq!(replay(&state).unwrap(), second);
    }
    #[test]
    fn rejects_unreachable_and_oversized_state_and_unexpected_receipts() {
        let mut state = State {
            seed: 0,
            ..State::default()
        };
        assert!(replay(&state).is_err());
        state = State::default();
        state.moves = vec![Direction::Left; MAX_MOVES + 1];
        assert!(replay(&state).is_err());
        state = State::default();
        let mut game = Game::new(state.seed);
        while game.step(Direction::Left) {
            state.moves.push(Direction::Left);
        }
        state.moves.push(Direction::Left);
        assert!(replay(&state).is_err());
        assert!(process(request(&State::default(), "host-result")).is_err());
    }
    #[test]
    fn record_save_uses_actual_host_revision_and_waits_for_receipt() {
        let mut input = request(&State::default(), "save-best");
        input["host"] =
            json!({"permissions":["storage"],"storage":{"revision":7,"values":{"bestScore":100}}});
        let view = process(input).unwrap();
        assert_eq!(view["effects"][0]["expectedRevision"], 7);
        assert_eq!(view["effects"][0]["changes"]["bestScore"], 100);
        let mut next = json!({"version":2,"locale":"en","state":view["state"],"fields":{},"action":"host-result","results":[{"id":"save-best","value":{"revision":8,"values":{"bestScore":100}},"error":null}],"host":{"permissions":["storage"],"storage":{"revision":8,"values":{"bestScore":100}}}});
        let saved = process(next.clone()).unwrap();
        assert!(saved["description"].as_str().unwrap().contains("saved"));
        assert_eq!(saved["effects"], json!([]));
        next["results"][0]["error"] =
            json!({"code":"CONFLICT","message":"conflict","retryable":false});
        assert!(
            process(next).unwrap()["description"]
                .as_str()
                .unwrap()
                .contains("Another session")
        );
    }
}
