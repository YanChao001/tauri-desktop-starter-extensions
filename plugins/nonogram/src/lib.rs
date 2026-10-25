//! Small picture puzzles; their row/column clues have exactly one solution.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MAX_HISTORY: usize = 1024;
const PUZZLES: [[u8; 5]; 3] = [
    [0b01010, 0b11111, 0b11111, 0b01110, 0b00100],
    [0b00100, 0b01110, 0b11111, 0b00100, 0b00100],
    [0b00100, 0b01110, 0b11111, 0b01110, 0b00100],
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Tool {
    Fill,
    Cross,
}
#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Notice {
    #[default]
    Ready,
    Changed,
    Undone,
    Reset,
    Hint,
    Correct,
    Incorrect,
    Incomplete,
    Saved,
    Loaded,
    SaveFailed,
    Conflict,
    NoStorage,
    InvalidSaved,
    Unknown,
    Limit,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mark {
    cell: usize,
    value: u8,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u8,
    puzzle: usize,
    history: Vec<Mark>,
    tool: Tool,
    notice: Notice,
    pending: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: 1,
            puzzle: 0,
            history: vec![],
            tool: Tool::Fill,
            notice: Notice::Ready,
            pending: false,
        }
    }
}
fn board(state: &State) -> Result<[u8; 25], String> {
    if state.schema != 1 || state.puzzle >= PUZZLES.len() || state.history.len() > MAX_HISTORY {
        return Err("Invalid nonogram state / 无效数织状态".into());
    }
    let mut cells = [0; 25];
    for mark in &state.history {
        if mark.cell >= 25 || mark.value > 2 || cells[mark.cell] == mark.value {
            return Err("Unreachable mark history / 标记历史不可达".into());
        }
        cells[mark.cell] = mark.value;
    }
    Ok(cells)
}
fn solution(puzzle: usize) -> [bool; 25] {
    std::array::from_fn(|i| PUZZLES[puzzle][i / 5] & (1 << (4 - i % 5)) != 0)
}
fn clues(line: &[bool]) -> Vec<usize> {
    let mut result = vec![];
    let mut count = 0;
    for filled in line.iter().copied().chain([false]) {
        if filled {
            count += 1;
        } else if count > 0 {
            result.push(count);
            count = 0;
        }
    }
    if result.is_empty() {
        result.push(0);
    }
    result
}
fn all_clues(puzzle: usize) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let answer = solution(puzzle);
    let rows = (0..5).map(|r| clues(&answer[r * 5..r * 5 + 5])).collect();
    let columns = (0..5)
        .map(|c| clues(&(0..5).map(|r| answer[r * 5 + c]).collect::<Vec<_>>()))
        .collect();
    (rows, columns)
}
fn completed(cells: &[u8; 25], puzzle: usize) -> bool {
    cells
        .iter()
        .zip(solution(puzzle))
        .all(|(cell, answer)| (*cell == 1) == answer)
}
fn wrong(cells: &[u8; 25], puzzle: usize) -> bool {
    cells
        .iter()
        .zip(solution(puzzle))
        .any(|(cell, answer)| (*cell == 1 && !answer) || (*cell == 2 && answer))
}
fn tr<'a>(zh: bool, cn: &'a str, en: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn title(puzzle: usize, zh: bool) -> &'static str {
    match puzzle {
        0 => tr(zh, "爱心", "Heart"),
        1 => tr(zh, "小树", "Little tree"),
        _ => tr(zh, "钻石", "Diamond"),
    }
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
    let storage = &host["storage"];
    storage["revision"]
        .as_i64()
        .filter(|r| (0..=i32::MAX as i64).contains(r))?;
    storage["values"].as_object()?;
    Some(storage)
}
fn saved(storage: &Value) -> Option<State> {
    let value = &storage["values"]["progress"];
    let raw = serde_json::to_string(value).ok()?;
    if raw.len() > 40_000 {
        return None;
    }
    let state: State = serde_json::from_value(value.clone()).ok()?;
    if state.pending || board(&state).is_err() {
        return None;
    }
    Some(state)
}
pub fn process(request: Value) -> Result<Value, String> {
    if request["version"] != 2 || !matches!(request["locale"].as_str(), Some("zh-CN" | "en")) {
        return Err("Unsupported view request / 不支持的视图请求".into());
    }
    let zh = request["locale"] == "zh-CN";
    let raw = request["state"]
        .as_str()
        .ok_or("Missing state / 缺少状态")?;
    if raw.len() > 40_000 {
        return Err("Nonogram state too large / 数织状态过大".into());
    }
    let mut state: State = if raw.is_empty() {
        State::default()
    } else {
        serde_json::from_str(raw).map_err(|_| "Invalid nonogram state / 无效数织状态")?
    };
    let mut cells = board(&state)?;
    let snapshot = storage(&request);
    let current_action = request["action"].as_str().unwrap_or("");
    if state.pending && current_action != "host-result" {
        return Err("Missing storage receipt / 缺少存储回执".into());
    }
    if let Some(tool) = request["fields"]["tool"].as_str() {
        state.tool = match tool {
            "fill" => Tool::Fill,
            "cross" => Tool::Cross,
            _ => return Err("Invalid marking tool / 无效标记工具".into()),
        };
    }
    let mut effects = vec![];
    match current_action {
        "" => {}
        "undo" => {
            if state.history.pop().is_some() {
                state.notice = Notice::Undone;
            }
        }
        "reset" => {
            state.history.clear();
            state.notice = Notice::Reset;
        }
        "hint" => {
            let answer = solution(state.puzzle);
            if state.history.len() >= MAX_HISTORY {
                state.notice = Notice::Limit;
            } else {
                let conflict = cells
                    .iter()
                    .zip(answer)
                    .position(|(cell, filled)| (*cell == 1 && !filled) || (*cell == 2 && filled));
                let unknown = cells.iter().position(|cell| *cell == 0);
                if let Some(cell) = conflict.or(unknown) {
                    state.history.push(Mark {
                        cell,
                        value: if answer[cell] { 1 } else { 2 },
                    });
                    state.notice = Notice::Hint;
                } else {
                    state.notice = Notice::Correct;
                }
            }
        }
        "check" => {
            state.notice = if wrong(&cells, state.puzzle) {
                Notice::Incorrect
            } else if completed(&cells, state.puzzle) {
                Notice::Correct
            } else {
                Notice::Incomplete
            };
        }
        "save" => {
            if let Some(snapshot) = snapshot {
                let mut progress = state.clone();
                progress.pending = false;
                progress.notice = Notice::Ready;
                effects.push(json!({"kind":"storage","id":"save-progress","expectedRevision":snapshot["revision"],"changes":{"progress":progress}}));
                state.pending = true;
            } else {
                state.notice = Notice::NoStorage;
            }
        }
        "load" => {
            if let Some(snapshot) = snapshot {
                if let Some(progress) = saved(snapshot) {
                    state = progress;
                    state.notice = Notice::Loaded;
                } else {
                    state.notice = Notice::InvalidSaved;
                }
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
                .ok_or("Missing storage receipt")?;
            if results.len() != 1 || results[0]["id"] != "save-progress" {
                return Err("Invalid storage receipt / 无效存储回执".into());
            }
            state.pending = false;
            state.notice = if results[0]["error"].is_null() {
                Notice::Saved
            } else if results[0]["error"]["code"] == "CONFLICT" {
                Notice::Conflict
            } else {
                Notice::SaveFailed
            };
        }
        _ => {
            if let Some(index) = current_action
                .strip_prefix("puzzle-")
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|i| *i < PUZZLES.len())
            {
                state = State {
                    puzzle: index,
                    ..State::default()
                };
            } else if let Some(cell) = current_action
                .strip_prefix("cell-")
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|i| *i < 25)
            {
                if state.history.len() >= MAX_HISTORY {
                    state.notice = Notice::Limit;
                } else {
                    let value = match state.tool {
                        Tool::Fill => 1,
                        Tool::Cross => 2,
                    };
                    state.history.push(Mark {
                        cell,
                        value: if cells[cell] == value { 0 } else { value },
                    });
                    state.notice = Notice::Changed;
                }
            } else {
                state.notice = Notice::Unknown;
            }
        }
    }
    cells = board(&state)?;
    let (rows, columns) = all_clues(state.puzzle);
    let status = match state.notice {
        Notice::Ready => tr(
            zh,
            "读行列线索，画出藏起来的小图案。",
            "Read the row and column clues to reveal a tiny picture.",
        ),
        Notice::Changed => tr(
            zh,
            "已更新标记。相同工具再次点击会清除该格。",
            "Mark updated. Click again with the same tool to clear it.",
        ),
        Notice::Undone => tr(
            zh,
            "已撤销最后一次标记或提示。",
            "The last mark or hint was undone.",
        ),
        Notice::Reset => tr(
            zh,
            "本题已重置；已保存进度不会被自动覆盖。",
            "Puzzle reset; saved progress was not overwritten.",
        ),
        Notice::Hint => tr(
            zh,
            "提示已修正一个错误标记或揭示一个未知格。提示使用本题唯一解，可撤销。",
            "A hint corrected a conflicting mark or revealed one unknown cell. Hints use the unique solution and can be undone.",
        ),
        Notice::Correct => tr(
            zh,
            "完成！所有填充格都符合行列线索。空白格无需全部打叉。",
            "Solved! Every filled cell matches the clues. Empty cells do not all need crosses.",
        ),
        Notice::Incorrect => tr(
            zh,
            "还有错误的填充或叉号。试试撤销，或用提示修正一个格子。",
            "Some filled cells or crosses conflict with the clues. Undo or use a hint to correct one cell.",
        ),
        Notice::Incomplete => tr(
            zh,
            "当前已标记的格子没有错误，仍有需要填充的格子。",
            "Your current marks are correct; some filled cells are still missing.",
        ),
        Notice::Saved => tr(
            zh,
            "进度已保存到此插件的私有存储。",
            "Progress saved to this plugin's private storage.",
        ),
        Notice::Loaded => tr(
            zh,
            "已恢复保存的题目与操作历史。",
            "The saved puzzle and move history were restored.",
        ),
        Notice::SaveFailed => tr(
            zh,
            "保存失败，未自动重试；当前棋盘仍保留。",
            "Save failed without automatic retry; this board was kept.",
        ),
        Notice::Conflict => tr(
            zh,
            "另一会话已更新存储，请先核对再手动保存。",
            "Another session changed storage. Review it before saving again manually.",
        ),
        Notice::NoStorage => tr(
            zh,
            "未授权私有存储；仍可游玩，并通过宿主导出会话。",
            "Private storage is not granted. Keep playing or export a session through the host.",
        ),
        Notice::InvalidSaved => tr(
            zh,
            "没有可读取的保存进度，当前棋盘保持不变。",
            "No valid saved progress was found; the current board was kept.",
        ),
        Notice::Unknown => tr(
            zh,
            "不支持此操作，棋盘保持原样。",
            "Unsupported action; the board was kept.",
        ),
        Notice::Limit => tr(
            zh,
            "达到 1024 次标记上限，可撤销或重置本题。",
            "The 1024-mark session limit was reached. Undo or reset this puzzle.",
        ),
    };
    let clue_text = |numbers: &Vec<usize>| {
        numbers
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let mut board_actions = vec![action("corner", "↘", true, false)];
    for (c, clue) in columns.iter().enumerate() {
        let mut a = action(&format!("col-{c}"), &clue_text(clue), true, false);
        a["ariaLabel"] = json!(format!(
            "{} {}: {}",
            tr(zh, "第列线索", "Column clue"),
            c + 1,
            clue_text(clue)
        ));
        board_actions.push(a);
    }
    for (r, clue) in rows.iter().enumerate() {
        let mut a = action(&format!("row-{r}"), &clue_text(clue), true, false);
        a["ariaLabel"] = json!(format!(
            "{} {}: {}",
            tr(zh, "第行线索", "Row clue"),
            r + 1,
            clue_text(clue)
        ));
        board_actions.push(a);
        for c in 0..5 {
            let cell = r * 5 + c;
            let label = match cells[cell] {
                1 => "■",
                2 => "×",
                _ => "·",
            };
            let mut a = action(&format!("cell-{cell}"), label, false, cells[cell] == 1);
            a["ariaLabel"] = json!(format!(
                "{} {}, {} {}, {}",
                tr(zh, "行", "Row"),
                r + 1,
                tr(zh, "列", "column"),
                c + 1,
                match cells[cell] {
                    1 => tr(zh, "已填充", "filled"),
                    2 => tr(zh, "已标空", "marked empty"),
                    _ => tr(zh, "未知", "unknown"),
                }
            ));
            board_actions.push(a);
        }
    }
    let tabs: Vec<_> = (0..3)
        .map(|i| json!({"id":format!("puzzle-{i}"),"label":title(i,zh),"active":state.puzzle==i}))
        .collect();
    Ok(
        json!({"version":2,"state":serde_json::to_string(&state).map_err(|e|e.to_string())?,"display":format!("{} · {}",tr(zh,"数织画室","Nonogram Studio"),title(state.puzzle,zh)),"description":status,"tabs":tabs,
        "fields":[{"id":"tool","label":tr(zh,"点击格子的工具","Cell marking tool"),"kind":"select","value":match state.tool{Tool::Fill=>"fill",Tool::Cross=>"cross"},"options":[{"value":"fill","label":tr(zh,"■ 填充","■ Fill")},{"value":"cross","label":tr(zh,"× 标为空白","× Mark empty")}]}],
        "groups":[{"columns":6,"board":true,"actions":board_actions},{"columns":4,"actions":[action("undo",tr(zh,"↶ 撤销","↶ Undo"),state.history.is_empty(),false),action("hint",tr(zh,"揭示一格","Reveal a cell"),false,false),action("check",tr(zh,"校验答案","Check answer"),false,true),action("reset",tr(zh,"重置本题","Reset puzzle"),false,false)]},{"columns":2,"actions":[action("save",tr(zh,"保存进度","Save progress"),snapshot.is_none(),false),action("load",tr(zh,"恢复已存进度","Load saved progress"),snapshot.is_none()||snapshot.and_then(saved).is_none(),false)]}],
        "blocks":[{"kind":"metrics","items":[{"label":tr(zh,"已标记","Marked"),"value":format!("{}/25",cells.iter().filter(|n|**n!=0).count())},{"label":tr(zh,"操作数","Moves"),"value":state.history.len().to_string()},{"label":tr(zh,"状态","Status"),"value":if completed(&cells,state.puzzle){tr(zh,"已完成","Solved")}else{tr(zh,"解谜中","In progress")}}]},
        {"kind":"markdown","text":tr(zh,"**玩法**：左侧数字是每行从左到右连续填充的长度；上方数字是每列从上到下的长度。多段之间至少留一格空白。例如 `2 · 1` 表示连续填 2 格，空至少 1 格，再填 1 格。\n\n先选择 **■ 填充** 或 **× 标为空白**，再点击格子；重复点击同标记可清除。三题都经过唯一解校验；提示会揭示答案，不计入无提示挑战。**校验答案** 会区分错误与未完成，空格不强制打叉。\n\n保存 / 恢复使用显式授权的插件私有存储，不自动覆盖。切换题目会开始该题的新棋盘，先保存或导出会话可保留当前进度。","**How to play:** Numbers at the left describe consecutive filled runs from left to right; numbers above describe runs from top to bottom. Separate runs by at least one empty cell. `2 · 1` means two filled cells, at least one empty cell, then one filled cell.\n\nSelect **■ Fill** or **× Mark empty**, then click cells. Repeating the same mark clears a cell. All three puzzles have a verified unique solution; hints reveal an answer cell. **Check answer** distinguishes mistakes from an incomplete board; crosses are optional.\n\nSave / load uses explicitly granted private storage and never overwrites automatically. Switching puzzles starts a fresh board; save or export a session first to keep progress.")}],"effects":effects}),
    )
}

#[cfg(target_arch = "wasm32")]
#[extism_pdk::plugin_fn]
pub fn play(input: String) -> extism_pdk::FnResult<String> {
    let view = process(serde_json::from_str(&input)?).map_err(extism_pdk::Error::msg)?;
    Ok(serde_json::to_string(&view)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(state: &State, action: &str) -> Value {
        json!({"version":2,"locale":"en","state":serde_json::to_string(state).unwrap(),"fields":{},"action":action})
    }
    fn unique_count(puzzle: usize) -> usize {
        let (rows, columns) = all_clues(puzzle);
        let candidates: Vec<Vec<u8>> = rows
            .iter()
            .map(|clue| {
                (0..32)
                    .filter(|bits| {
                        clues(
                            &(0..5)
                                .map(|c| bits & (1 << (4 - c)) != 0)
                                .collect::<Vec<_>>(),
                        ) == *clue
                    })
                    .collect()
            })
            .collect();
        fn search(
            row: usize,
            grid: &mut [u8; 5],
            candidates: &[Vec<u8>],
            columns: &[Vec<usize>],
            count: &mut usize,
        ) {
            if row == 5 {
                if (0..5).all(|c| {
                    clues(
                        &(0..5)
                            .map(|r| grid[r] & (1 << (4 - c)) != 0)
                            .collect::<Vec<_>>(),
                    ) == columns[c]
                }) {
                    *count += 1;
                }
                return;
            }
            for bits in &candidates[row] {
                grid[row] = *bits;
                search(row + 1, grid, candidates, columns, count);
            }
        }
        let mut count = 0;
        search(0, &mut [0; 5], &candidates, &columns, &mut count);
        count
    }
    #[test]
    fn every_puzzle_has_exactly_one_solution_for_its_clues() {
        for puzzle in 0..PUZZLES.len() {
            assert_eq!(unique_count(puzzle), 1, "puzzle {puzzle}");
        }
    }
    #[test]
    fn clue_runs_keep_separation_and_empty_lines() {
        assert_eq!(clues(&[true, true, false, true, false]), vec![2, 1]);
        assert_eq!(clues(&[false; 5]), vec![0]);
    }
    #[test]
    fn cross_toggle_and_undo_restore_previous_mark() {
        let state = State::default();
        let view = process(request(&state, "cell-2")).unwrap();
        let marked: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(board(&marked).unwrap()[2], 1);
        let mut input = request(&marked, "cell-2");
        input["fields"] = json!({"tool":"cross"});
        let crossed: State =
            serde_json::from_str(process(input).unwrap()["state"].as_str().unwrap()).unwrap();
        assert_eq!(board(&crossed).unwrap()[2], 2);
        let undo: State = serde_json::from_str(
            process(request(&crossed, "undo")).unwrap()["state"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(board(&undo).unwrap()[2], 1);
        let cleared: State = serde_json::from_str(
            process(request(&marked, "cell-2")).unwrap()["state"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(board(&cleared).unwrap()[2], 0);
    }
    #[test]
    fn hints_correct_mistakes_and_eventually_solve_all_puzzles() {
        for puzzle in 0..3 {
            let mut state = State {
                puzzle,
                ..State::default()
            };
            let answer = solution(puzzle);
            let wrong_cell = answer.iter().position(|filled| !*filled).unwrap();
            state.history.push(Mark {
                cell: wrong_cell,
                value: 1,
            });
            for _ in 0..25 {
                let view = process(request(&state, "hint")).unwrap();
                state = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
            }
            assert!(completed(&board(&state).unwrap(), puzzle));
            assert!(!wrong(&board(&state).unwrap(), puzzle));
        }
    }
    #[test]
    fn completed_board_does_not_require_crosses() {
        for puzzle in 0..3 {
            let cells = solution(puzzle).map(u8::from);
            assert!(completed(&cells, puzzle));
            let mut bad = cells;
            let empty = bad.iter().position(|cell| *cell == 0).unwrap();
            bad[empty] = 1;
            assert!(!completed(&bad, puzzle));
            assert!(wrong(&bad, puzzle));
        }
    }
    #[test]
    fn rejects_unknown_state_fields_unreachable_marks_and_invalid_saved_data() {
        let mut state = State::default();
        state.history.push(Mark { cell: 25, value: 1 });
        assert!(board(&state).is_err());
        state.history = vec![Mark { cell: 0, value: 1 }, Mark { cell: 0, value: 1 }];
        assert!(board(&state).is_err());
        state.history = vec![Mark { cell: 0, value: 3 }];
        assert!(board(&state).is_err());
        let mut input = request(&State::default(), "");
        input["state"] = json!(
            "{\"schema\":1,\"puzzle\":0,\"history\":[],\"tool\":\"fill\",\"notice\":\"ready\",\"pending\":false,\"extra\":true}"
        );
        assert!(process(input).is_err());
        assert!(saved(&json!({"values":{"progress":{"schema":99}}})).is_none());
    }
    #[test]
    fn save_and_load_preserve_progress_and_cas_revision() {
        let state = State {
            history: vec![Mark { cell: 0, value: 2 }],
            ..State::default()
        };
        let mut input = request(&state, "save");
        input["host"] = json!({"permissions":["storage"],"storage":{"revision":4,"values":{}}});
        let output = process(input).unwrap();
        assert_eq!(output["effects"][0]["expectedRevision"], 4);
        let progress = output["effects"][0]["changes"]["progress"].clone();
        let mut load = request(&State::default(), "load");
        load["host"] = json!({"permissions":["storage"],"storage":{"revision":5,"values":{"progress":progress}}});
        let restored: State =
            serde_json::from_str(process(load).unwrap()["state"].as_str().unwrap()).unwrap();
        assert_eq!(board(&restored).unwrap()[0], 2);
    }
}
