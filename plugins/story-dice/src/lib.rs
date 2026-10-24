//! Offline, deterministic writing prompts. The host provides UI and optional CAS storage only.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const DRAFT_LIMIT: usize = 24_000;
const MAX_FAVORITES: usize = 8;
const MAX_HISTORY: usize = 8;
const STORAGE_KEY: &str = "story-favorites";
type Pair = (&'static str, &'static str);

// Each genre has 8 choices in 5 dimensions: 32,768 combinations before drafting.
const DECKS: [[[Pair; 8]; 5]; 3] = [
    [
        [
            ("迷路的地图师", "a lost cartographer"),
            ("退休的屠龙者", "a retired dragon slayer"),
            ("失去魔力的学徒", "an apprentice without magic"),
            ("替人送梦的邮差", "a dream-delivery courier"),
            ("害怕黑暗的幽灵", "a ghost afraid of the dark"),
            ("会读心的园丁", "a mind-reading gardener"),
            ("不肯预言的先知", "a reluctant prophet"),
            ("偷走自己名字的盗贼", "a thief who stole their own name"),
        ],
        [
            ("漂浮的图书馆", "a floating library"),
            ("只在雨中出现的市集", "a market that appears only in rain"),
            ("巨兽背上的村庄", "a village on a giant beast"),
            ("遗忘之海的灯塔", "a lighthouse on the sea of forgetting"),
            ("禁止说谎的旅馆", "an inn where lying is impossible"),
            ("倒着流的瀑布", "a waterfall flowing upward"),
            ("封印着夏天的雪山", "a snowy mountain imprisoning summer"),
            ("藏在茶杯里的王国", "a kingdom inside a teacup"),
        ],
        [
            (
                "必须归还一段不属于自己的记忆",
                "must return a memory that is not theirs",
            ),
            (
                "要在日出前修复一颗星星",
                "must repair a star before sunrise",
            ),
            (
                "发现魔王是唯一讲真话的人",
                "discovers the dark lord alone tells the truth",
            ),
            (
                "需要与宿敌交换身份一天",
                "must swap identities with a rival for a day",
            ),
            (
                "必须决定谁能听到最后一个愿望",
                "must choose who hears the last wish",
            ),
            (
                "发现每次施法都会抹去一个词",
                "discovers every spell erases a word",
            ),
            (
                "要说服巨龙归还偷走的春天",
                "must persuade a dragon to return spring",
            ),
            (
                "收到一封自己明天才会写的信",
                "receives a letter they will write tomorrow",
            ),
        ],
        [
            ("一枚指向家乡的硬币", "a coin that points toward home"),
            ("能保存笑声的瓶子", "a bottle that stores laughter"),
            ("永远慢一分钟的怀表", "a watch always one minute late"),
            ("害怕火焰的火柴", "matches afraid of fire"),
            ("画不出主人的镜子", "a mirror that cannot show its owner"),
            (
                "记得每次告别的围巾",
                "a scarf that remembers every farewell",
            ),
            ("不肯打开的钥匙", "a key that refuses to unlock"),
            ("长出羽毛的石头", "a stone growing feathers"),
        ],
        [
            (
                "而真正的宝藏是一句道歉",
                "but the real treasure is an apology",
            ),
            (
                "然而反派也在拯救同一个人",
                "but the antagonist is saving the same person",
            ),
            (
                "而地图上的终点会不断移动",
                "but the destination on the map keeps moving",
            ),
            (
                "然而只有失败才能解除诅咒",
                "but only failure can break the curse",
            ),
            (
                "而旁白也迷失在故事里",
                "but the narrator is lost in the story too",
            ),
            (
                "然而所有人都忘了约定的原因",
                "but everyone forgot why the promise was made",
            ),
            (
                "而答案藏在一句儿歌里",
                "but the answer hides in a nursery rhyme",
            ),
            (
                "然而离开就会失去所有魔法",
                "but leaving will cost all magic",
            ),
        ],
    ],
    [
        [
            ("维修记忆的工程师", "a memory repair engineer"),
            (
                "从未见过地球的宇航员",
                "an astronaut who has never seen Earth",
            ),
            ("想学做饭的机器人", "a robot learning to cook"),
            ("为黑洞配音的演员", "an actor voicing black holes"),
            (
                "收集废弃卫星的孩子",
                "a child collecting abandoned satellites",
            ),
            ("最后一位纸质书管理员", "the last paper-book librarian"),
            ("辞职的行星导航员", "a planetary navigator who quit"),
            ("负责外星礼仪的翻译", "a translator of alien etiquette"),
        ],
        [
            ("即将关停的空间站", "a space station about to close"),
            ("复制了三次的城市", "a city copied three times"),
            ("没有影子的月球基地", "a moon base without shadows"),
            ("由植物驾驶的飞船", "a ship piloted by plants"),
            ("正在做梦的数据中心", "a dreaming data center"),
            ("太阳背面的便利店", "a convenience store behind the sun"),
            ("储存旧天气的仓库", "a warehouse of old weather"),
            ("禁止联网的未来博物馆", "a future museum with no network"),
        ],
        [
            (
                "收到来自消失星球的求救",
                "receives a distress call from a vanished planet",
            ),
            (
                "必须证明自己不是自己的备份",
                "must prove they are not their own backup",
            ),
            (
                "发现时间旅行只会改变食谱",
                "discovers time travel changes only recipes",
            ),
            (
                "要在断电前保存一场梦",
                "must save a dream before the power fails",
            ),
            (
                "需要教人工智能如何告别",
                "must teach an AI how to say goodbye",
            ),
            (
                "发现故障其实是一种外星语言",
                "discovers a malfunction is an alien language",
            ),
            (
                "必须退回一份跨越百年的快递",
                "must return a delivery a century late",
            ),
            (
                "被要求为两个太阳制定作息",
                "must create a schedule for two suns",
            ),
        ],
        [
            ("一张没有拍摄过的照片", "a photograph never taken"),
            (
                "只会说谢谢的翻译器",
                "a translator that only says thank you",
            ),
            ("装着海风的硬盘", "a drive containing sea breezes"),
            ("来自未来的旧收据", "an old receipt from the future"),
            ("有自己意见的宇航服", "a spacesuit with opinions"),
            ("写满人类笑话的芯片", "a chip full of human jokes"),
            (
                "不会遗忘的断电闹钟",
                "an unplugged alarm clock that remembers",
            ),
            (
                "只测量孤独的探测器",
                "a detector that measures only loneliness",
            ),
        ],
        [
            (
                "然而信号来自自己的童年",
                "but the signal comes from their own childhood",
            ),
            (
                "而整个任务只是一次生日惊喜",
                "but the mission is a birthday surprise",
            ),
            (
                "然而外星人早已学会了人类的俚语",
                "but the aliens already know human slang",
            ),
            (
                "而最可靠的数据是一首歌",
                "but the most reliable data is a song",
            ),
            (
                "然而机器比人更珍惜旧物",
                "but machines cherish old things more than people",
            ),
            (
                "而回程需要舍弃所有坐标",
                "but returning requires giving up every coordinate",
            ),
            (
                "然而他们正在成为别人的传说",
                "but they are becoming someone else's legend",
            ),
            (
                "而拯救世界只需按下取消键",
                "but saving the world requires pressing cancel",
            ),
        ],
    ],
    [
        [
            ("准备转行的咖啡师", "a barista changing careers"),
            ("失眠的夜班司机", "an insomniac night-shift driver"),
            ("想交朋友的退休侦探", "a retired detective seeking friends"),
            ("修伞的手艺人", "an umbrella repairer"),
            (
                "给陌生人写诗的学生",
                "a student writing poems for strangers",
            ),
            ("不识路的导游", "a tour guide with no sense of direction"),
            ("在屋顶种菜的厨师", "a chef growing vegetables on a rooftop"),
            (
                "寻找第一位客人的店主",
                "a shopkeeper seeking a first customer",
            ),
        ],
        [
            ("即将搬迁的旧书店", "an old bookshop about to move"),
            ("每周只开一天的车站", "a station open one day a week"),
            ("停电的社区厨房", "a community kitchen during a blackout"),
            (
                "总有人留灯的公寓",
                "an apartment where someone leaves a light on",
            ),
            ("雨后的露天电影院", "an outdoor cinema after rain"),
            (
                "贴满寻物启事的巷子",
                "an alley full of lost-and-found notices",
            ),
            ("没有菜单的小餐馆", "a little restaurant with no menu"),
            ("开往终点站的末班车", "the last bus to the end of the line"),
        ],
        [
            (
                "想找到一封没有署名的感谢信的主人",
                "seeks the author of an unsigned thank-you letter",
            ),
            (
                "要为陌生人办一场五分钟的婚礼",
                "must arrange a five-minute wedding for strangers",
            ),
            (
                "被误认为早已失联的朋友",
                "is mistaken for a long-lost friend",
            ),
            (
                "需要在关门前归还借了十年的东西",
                "must return something borrowed ten years ago before closing",
            ),
            (
                "想帮邻居实现一个很小的愿望",
                "wants to fulfill a neighbor's tiny wish",
            ),
            (
                "被迫和最合不来的人共用一把伞",
                "must share an umbrella with their least favorite person",
            ),
            (
                "发现熟悉的路突然少了一家店",
                "finds a familiar street missing a shop",
            ),
            (
                "收到一张已经过期的邀请函",
                "receives an invitation that has expired",
            ),
        ],
        [
            ("一把刻着日期的钥匙", "a key engraved with a date"),
            ("一盒不同口味的糖", "a box of mismatched sweets"),
            ("录着街道声音的磁带", "a cassette of street sounds"),
            ("只剩一角的合影", "the corner of a group photograph"),
            ("一张写错目的地的车票", "a ticket to the wrong destination"),
            ("缝了三种颜色的手套", "a glove stitched in three colors"),
            ("没有收件人的明信片", "a postcard without an address"),
            ("装着种子的药瓶", "a medicine bottle full of seeds"),
        ],
        [
            ("而那个人就住在隔壁", "but that person lives next door"),
            (
                "然而最珍贵的是一起等雨停的时间",
                "but the treasure is time spent waiting for rain to stop",
            ),
            (
                "而误会让两个人成了搭档",
                "but the misunderstanding makes them partners",
            ),
            (
                "然而大家早就悄悄准备好了",
                "but everyone has already quietly prepared",
            ),
            (
                "而故事的开头其实是结尾",
                "but the beginning was actually the ending",
            ),
            (
                "然而那件丢失的东西从未离开",
                "but the lost object never left",
            ),
            (
                "而最需要帮助的人一直在帮别人",
                "but the one needing help has been helping others",
            ),
            (
                "然而今天恰好是重新开始的好日子",
                "but today happens to be a good day to start again",
            ),
        ],
    ],
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Roll {
    seed: String,
    turn: u32,
    genre: String,
    dice: [u8; 5],
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Favorite {
    title: String,
    draft: String,
    roll: Roll,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Favorites {
    version: u8,
    items: Vec<Favorite>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    version: u8,
    roll: Roll,
    locks: [bool; 5],
    next_turn: String,
    history: Vec<Roll>,
    title: String,
    draft: String,
    pending: bool,
    favorites_revision: Option<i32>,
    status: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    revision: i32,
    values: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    permissions: Vec<String>,
    storage: Option<Snapshot>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptError {
    code: String,
    message: String,
    retryable: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    id: String,
    value: Option<Value>,
    error: Option<ReceiptError>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    action: Option<String>,
    state: String,
    fields: BTreeMap<String, String>,
    locale: String,
    #[serde(default)]
    host: Option<Host>,
    #[serde(default)]
    results: Vec<Receipt>,
    #[serde(default)]
    data: Option<Value>,
}

fn text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.contains('\0')
}
fn single_line(value: &str, maximum: usize) -> bool {
    text(value, maximum) && !value.contains(['\r', '\n'])
}
fn genre_index(value: &str) -> Option<usize> {
    ["fantasy", "scifi", "cozy"]
        .iter()
        .position(|v| *v == value)
}
fn valid_roll(roll: &Roll) -> bool {
    single_line(&roll.seed, 80)
        && !roll.seed.trim().is_empty()
        && (1..=100_000).contains(&roll.turn)
        && genre_index(&roll.genre).is_some()
        && roll.dice.iter().all(|value| *value < 8)
}
fn valid_favorite(value: &Favorite) -> bool {
    single_line(&value.title, 120)
        && !value.title.trim().is_empty()
        && text(&value.draft, DRAFT_LIMIT)
        && valid_roll(&value.roll)
}
fn validate_state(state: &State) -> Result<(), String> {
    if state.version != 1
        || !valid_roll(&state.roll)
        || !single_line(&state.next_turn, 6)
        || state.history.len() > MAX_HISTORY
        || !state.history.iter().all(valid_roll)
        || !single_line(&state.title, 120)
        || !text(&state.draft, DRAFT_LIMIT)
        || !text(&state.status, 600)
        || state
            .favorites_revision
            .is_some_and(|revision| revision < 0)
    {
        return Err("Invalid story state; original session is unchanged".into());
    }
    Ok(())
}
fn rng(seed: &str, genre: &str, turn: u32) -> u64 {
    // Stable across architectures and compiler releases; not a security RNG.
    seed.bytes()
        .chain(genre.bytes())
        .chain(turn.to_le_bytes())
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}
fn throw(seed: &str, genre: &str, turn: u32) -> [u8; 5] {
    let mut value = rng(seed, genre, turn);
    std::array::from_fn(|_| {
        value = value.wrapping_add(0x9e3779b97f4a7c15);
        let mut mixed = value;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((mixed ^ (mixed >> 31)) >> 61) as u8
    })
}
fn tr<'a>(zh: bool, cn: &'a str, en: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn phrase(roll: &Roll, index: usize, zh: bool) -> &str {
    let pair = DECKS[genre_index(&roll.genre).unwrap()][index][roll.dice[index] as usize];
    if zh { pair.0 } else { pair.1 }
}
fn prompt(roll: &Roll, zh: bool) -> String {
    if zh {
        format!(
            "{}来到{}，{}。随身带着{}，{}。",
            phrase(roll, 0, zh),
            phrase(roll, 1, zh),
            phrase(roll, 2, zh),
            phrase(roll, 3, zh),
            phrase(roll, 4, zh)
        )
    } else {
        format!(
            "{} arrives at {} and {}. They carry {}, {}.",
            phrase(roll, 0, zh),
            phrase(roll, 1, zh),
            phrase(roll, 2, zh),
            phrase(roll, 3, zh),
            phrase(roll, 4, zh)
        )
    }
}
fn button(id: &str, label: &str, disabled: bool, primary: bool) -> Value {
    json!({"id":id,"label":label,"disabled":disabled,"primary":primary})
}
fn short_title(value: &str) -> String {
    if value.len() <= 80 {
        return value.into();
    }
    let mut end = 80;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}
fn field(id: &str, label: &str, value: &str, kind: &str, max_bytes: usize) -> Value {
    json!({"id":id,"label":label,"value":value,"kind":kind,"maxBytes":max_bytes})
}
fn favorites(host: Option<&Host>) -> Result<(Vec<Favorite>, Option<i32>), String> {
    let Some(host) = host else {
        return Ok((Vec::new(), None));
    };
    if !host.permissions.iter().any(|p| p == "storage") {
        return Ok((Vec::new(), None));
    }
    let snapshot = host
        .storage
        .as_ref()
        .ok_or("Storage grant has no host snapshot")?;
    if snapshot.revision < 0 {
        return Err("Invalid host storage revision".into());
    }
    let Some(value) = snapshot.values.get(STORAGE_KEY) else {
        return Ok((Vec::new(), Some(snapshot.revision)));
    };
    let stored: Favorites = serde_json::from_value(value.clone())
        .map_err(|_| "Saved favorites are invalid; preserve storage before recovery")?;
    if stored.version != 1
        || stored.items.len() > MAX_FAVORITES
        || !stored.items.iter().all(valid_favorite)
    {
        return Err("Saved favorites are incompatible; preserve storage before recovery".into());
    }
    Ok((stored.items, Some(snapshot.revision)))
}

/// The complete pure boundary is also used by native regression tests and smoke snapshots.
pub fn process(input: &str) -> Result<String, String> {
    if !text(input, 4 * 1024 * 1024) {
        return Err("Story request is too large or contains NUL".into());
    }
    let request: Request = serde_json::from_str(input).map_err(|_| "Invalid story request")?;
    if request.version != 2
        || !text(&request.state, 200_000)
        || !text(&request.locale, 32)
        || request.fields.len() > 10
        || request.results.len() > 1
    {
        return Err("Invalid story protocol or request bounds".into());
    }
    let zh = request.locale.starts_with("zh");
    let mut state: State = if request.state.is_empty() {
        let roll = Roll {
            seed: "hello-story".into(),
            turn: 1,
            genre: "fantasy".into(),
            dice: throw("hello-story", "fantasy", 1),
        };
        State {
            version: 1,
            draft: prompt(&roll, zh),
            roll,
            locks: [false; 5],
            next_turn: "2".into(),
            history: Vec::new(),
            title: tr(zh, "未命名的冒险", "Untitled adventure").into(),
            pending: false,
            favorites_revision: None,
            status: String::new(),
        }
    } else {
        serde_json::from_str(&request.state).map_err(|_| "Invalid story state")?
    };
    validate_state(&state)?;
    let previous_roll = state.roll.clone();
    let action = request.action.as_deref();
    if state.pending != (action == Some("host-result"))
        || (!request.results.is_empty() && !state.pending)
    {
        return Err("Unexpected story effect receipt; no automatic retry was performed".into());
    }
    for (key, value) in &request.fields {
        match key.as_str() {
            "seed" if single_line(value, 80) && !value.trim().is_empty() => {
                state.roll.seed = value.clone()
            }
            "genre" if genre_index(value).is_some() => {
                // A locked slot retains its choice within its current genre. Changing genre unlocks.
                if state.roll.genre != *value {
                    state.locks = [false; 5];
                }
                state.roll.genre = value.clone();
            }
            "turn" if single_line(value, 6) => state.next_turn = value.clone(),
            "title" if single_line(value, 120) => state.title = value.clone(),
            "draft" if text(value, DRAFT_LIMIT) => state.draft = value.clone(),
            key if key.starts_with("lock-") && ["true", "false"].contains(&value.as_str()) => {
                let index = key[5..]
                    .parse::<usize>()
                    .map_err(|_| "Invalid lock field")?;
                if index >= 5 || key != format!("lock-{index}") {
                    return Err("Invalid lock field".into());
                }
                state.locks[index] = value == "true";
            }
            _ => return Err("Unknown story field or field value exceeds its limit".into()),
        }
    }
    if state.roll.genre != previous_roll.genre {
        state.locks = [false; 5];
    }
    let (mut saved, revision) = favorites(request.host.as_ref())?;
    let list_changed = state.favorites_revision != revision;
    let mut effects = Vec::<Value>::new();
    let mut artifacts = Vec::<Value>::new();
    let mut errors = BTreeMap::<String, String>::new();
    let mut save = false;
    match action {
        None => {}
        Some("roll") => {
            if let Ok(turn) = state.next_turn.parse::<u32>() {
                if !(1..=100_000).contains(&turn) {
                    errors.insert(
                        "turn".into(),
                        tr(
                            zh,
                            "请输入 1–100000 的整数",
                            "Enter an integer from 1 to 100000",
                        )
                        .into(),
                    );
                } else {
                    state.history.insert(0, previous_roll);
                    state.history.truncate(MAX_HISTORY);
                    let dice = throw(&state.roll.seed, &state.roll.genre, turn);
                    for (i, value) in dice.into_iter().enumerate() {
                        if !state.locks[i] {
                            state.roll.dice[i] = value;
                        }
                    }
                    state.roll.turn = turn;
                    state.next_turn = (turn % 100_000 + 1).to_string();
                    state.status = tr(zh, "新灵感已生成。点击“写入草稿”才会替换当前草稿。", "New prompt ready. Use “Use prompt in draft” to replace your current draft.").into();
                }
            } else {
                errors.insert(
                    "turn".into(),
                    tr(
                        zh,
                        "请输入 1–100000 的整数",
                        "Enter an integer from 1 to 100000",
                    )
                    .into(),
                );
            }
        }
        Some("use-prompt") => {
            state.draft = prompt(&state.roll, zh);
            state.status = tr(zh, "灵感已写入草稿", "Prompt copied into draft").into();
        }
        Some("unlock") => state.locks = [false; 5],
        Some("export") => {
            if state.title.trim().is_empty() {
                errors.insert("title".into(), tr(zh, "请填写标题", "Enter a title").into());
            } else {
                // A fixed basename prevents a title from becoming a path or reserved filename.
                let body = format!(
                    "# {}\n\n{}\n\n---\n\n{}: {} · {}: {} · {}: {}\n\n{}\n",
                    state.title.replace(['\r', '\n'], " "),
                    state.draft,
                    tr(zh, "种子", "Seed"),
                    state.roll.seed.replace(['\r', '\n'], " "),
                    tr(zh, "序号", "Turn"),
                    state.roll.turn,
                    tr(zh, "题材", "Genre"),
                    state.roll.genre,
                    prompt(&state.roll, zh)
                );
                artifacts.push(json!({"name":"story-draft.md","mediaType":"text/markdown","encoding":"utf8","content":body}));
                state.status = tr(
                    zh,
                    "草稿已生成，请点击产物的保存按钮选择位置",
                    "Draft artifact ready; use its save button to choose a location",
                )
                .into();
            }
        }
        Some("save") => {
            if revision.is_none() {
                state.status = tr(
                    zh,
                    "收藏需要可选的私有存储授权；其他功能可以离线使用",
                    "Favorites need the optional storage grant; other features work offline",
                )
                .into();
            } else if state.title.trim().is_empty() {
                errors.insert("title".into(), tr(zh, "请填写标题", "Enter a title").into());
            } else if saved.len() >= MAX_FAVORITES {
                state.status = tr(
                    zh,
                    "最多收藏 8 篇，请先移除一篇",
                    "Up to 8 favorites; remove one before saving",
                )
                .into();
            } else {
                saved.push(Favorite {
                    title: state.title.clone(),
                    draft: state.draft.clone(),
                    roll: state.roll.clone(),
                });
                save = true;
            }
        }
        Some("host-result") => {
            let receipt = request
                .results
                .first()
                .ok_or("Missing story storage receipt")?;
            if receipt.id != "write-favorites"
                || (receipt.value.is_some() == receipt.error.is_some())
            {
                return Err("Invalid story storage receipt".into());
            }
            state.pending = false;
            if let Some(error) = &receipt.error {
                if !text(&error.code, 80) || !text(&error.message, 2000) {
                    return Err("Invalid storage error receipt".into());
                }
                let _retryable = error.retryable;
                state.status = format!(
                    "{} [{}]. {}",
                    tr(zh, "收藏操作未确认", "Favorite operation was not confirmed"),
                    error.code,
                    tr(
                        zh,
                        "查看当前收藏后决定是否重试；不会自动重试。",
                        "Review current favorites before deciding to retry; no automatic retry."
                    )
                );
            } else {
                let value = receipt.value.as_ref().unwrap();
                let confirmed = value["revision"]
                    .as_i64()
                    .ok_or("Invalid successful storage receipt")?;
                if confirmed < 0 || confirmed > i32::MAX as i64 || !value["values"].is_object() {
                    return Err("Invalid successful storage receipt".into());
                }
                state.status = tr(
                    zh,
                    "收藏变更已由宿主确认保存",
                    "Host confirmed the favorites change was saved",
                )
                .into();
            }
        }
        Some("host-data") => {
            let _data = request.data.as_ref();
            state.status = tr(
                zh,
                "故事骰子不读取工作台文件或文本；请直接编辑草稿",
                "Story Dice does not read workbench files or text; edit the draft directly",
            )
            .into();
        }
        Some(value) if value.starts_with("history-") => {
            let index = action_index(value, "history-", state.history.len())?;
            state.roll = state.history[index].clone();
            state.locks = [false; 5];
            state.next_turn = (state.roll.turn % 100_000 + 1).to_string();
            state.status = tr(
                zh,
                "已恢复历史灵感，草稿保持当前内容",
                "Historical prompt restored; current draft kept",
            )
            .into();
        }
        Some(value) if value.starts_with("favorite-") || value.starts_with("remove-") => {
            // Buttons refer to the previously displayed list, never today's item at that index.
            let removing = value.starts_with("remove-");
            let prefix = if removing { "remove-" } else { "favorite-" };
            let index = action_index(value, prefix, MAX_FAVORITES)?;
            if list_changed {
                state.status = tr(zh, "收藏已在另一窗口发生变化。已刷新列表并保留当前草稿，请重新选择。", "Favorites changed in another window. The list was refreshed and your draft kept; choose again.").into();
            } else if index >= saved.len() {
                return Err("Story favorite no longer exists".into());
            } else if removing {
                saved.remove(index);
                save = true;
            } else {
                state.roll = saved[index].roll.clone();
                state.title = saved[index].title.clone();
                state.draft = saved[index].draft.clone();
                state.locks = [false; 5];
                state.next_turn = (state.roll.turn % 100_000 + 1).to_string();
                state.status = tr(
                    zh,
                    "已载入收藏，替换当前草稿",
                    "Favorite loaded, replacing the current draft",
                )
                .into();
            }
        }
        Some(_) => return Err("Unknown story action".into()),
    }
    if save {
        effects.push(json!({"kind":"storage","id":"write-favorites","expectedRevision":revision.ok_or("Storage is not granted")?,"changes":{STORAGE_KEY:{"version":1,"items":saved}}}));
        state.pending = true;
        state.status = tr(
            zh,
            "正在请求宿主原子保存，尚未确认成功",
            "Requesting an atomic host save; success is not confirmed yet",
        )
        .into();
    }
    state.favorites_revision = revision;
    validate_state(&state)?;
    let labels = if zh {
        ["人物", "地点", "冲突", "物件", "转折"]
    } else {
        ["Character", "Place", "Conflict", "Object", "Twist"]
    };
    let mut fields = vec![
        field(
            "seed",
            tr(
                zh,
                "种子（同题材、序号和锁定状态可复现）",
                "Seed (replay with the same genre, turn and locks)",
            ),
            &state.roll.seed,
            "text",
            80,
        ),
        field(
            "turn",
            tr(zh, "下一次序号（1–100000）", "Next turn (1–100000)"),
            &state.next_turn,
            "text",
            6,
        ),
    ];
    fields.push(json!({"id":"genre","label":tr(zh,"题材（切换会解除锁定）","Genre (changing unlocks all slots)"),"kind":"select","value":state.roll.genre,"options":[{"value":"fantasy","label":tr(zh,"奇幻冒险","Fantasy")},{"value":"scifi","label":tr(zh,"温柔科幻","Gentle sci-fi")},{"value":"cozy","label":tr(zh,"生活小事","Cozy everyday")}]}));
    for (i, label) in labels.iter().enumerate() {
        fields.push(field(
            &format!("lock-{i}"),
            &format!("{} {}", tr(zh, "锁定", "Lock"), label),
            if state.locks[i] { "true" } else { "false" },
            "checkbox",
            5,
        ));
    }
    fields.push(field(
        "title",
        tr(zh, "标题", "Title"),
        &state.title,
        "text",
        120,
    ));
    fields.push(field(
        "draft",
        tr(
            zh,
            "写作草稿（24 KB；载入收藏或写入灵感会替换）",
            "Draft (24 KB; loading a favorite or prompt replaces it)",
        ),
        &state.draft,
        "textarea",
        DRAFT_LIMIT,
    ));
    let mut groups = vec![
        json!({"columns":3,"actions":[button("roll",tr(zh,"掷出新灵感","Roll new prompt"),false,true),button("use-prompt",tr(zh,"写入草稿（替换）","Use prompt in draft (replace)"),false,false),button("unlock",tr(zh,"解除全部锁定","Unlock all"),false,false),button("save",tr(zh,"收藏当前草稿","Favorite current draft"),revision.is_none() || saved.len() >= MAX_FAVORITES,false),button("export",tr(zh,"生成 Markdown 文件","Create Markdown file"),false,false)]}),
    ];
    if !state.history.is_empty() {
        groups.push(json!({"columns":4,"actions":state.history.iter().enumerate().map(|(i,roll)|button(&format!("history-{i}"),&format!("{} {} · #{}",tr(zh,"恢复灵感","Restore prompt"),i+1,roll.turn),false,false)).collect::<Vec<_>>()}));
    }
    if !saved.is_empty() {
        groups.push(json!({"columns":2,"actions":saved.iter().enumerate().flat_map(|(i,item)|[button(&format!("favorite-{i}"),&format!("{} {}",tr(zh,"载入（替换草稿）","Load (replace draft)"),short_title(&item.title)),false,false),button(&format!("remove-{i}"),&format!("{} {}",tr(zh,"移除收藏","Remove favorite"),i+1),false,false)]).collect::<Vec<_>>()}));
    }
    let mut blocks = vec![
        json!({"kind":"table","columns":[tr(zh,"维度","Dimension"),tr(zh,"灵感","Prompt"),tr(zh,"锁定","Locked")],"rows":(0..5).map(|i|vec![labels[i].to_owned(),phrase(&state.roll,i,zh).to_owned(),if state.locks[i]{tr(zh,"是","Yes")}else{tr(zh,"否","No")}.to_owned()]).collect::<Vec<_>>()}),
        json!({"kind":"markdown","text":format!("{}\n\n{}",prompt(&state.roll,zh),state.status)}),
        json!({"kind":"metrics","items":[{"label":tr(zh,"当前序号","Current turn"),"value":state.roll.turn.to_string()},{"label":tr(zh,"会话历史","Session history"),"value":state.history.len().to_string()},{"label":tr(zh,"私有收藏","Private favorites"),"value":format!("{}/{}",saved.len(),MAX_FAVORITES)}]}),
    ];
    if revision.is_none() {
        blocks.push(json!({"kind":"markdown","text":tr(zh,"收藏未开启：可在扩展详情授予可选 storage 权限。草稿和掷骰仍可使用；会话历史不是持久存储。","Favorites are off: grant optional storage in extension details. Drafting and dice still work; session history is not persistent storage.")}));
    }
    serde_json::to_string(&json!({"version":2,"state":serde_json::to_string(&state).map_err(|_|"Cannot encode story state")?,"display":tr(zh,"故事骰子","Story Dice"),"description":tr(zh,"锁住喜欢的片段，换一组意外。种子与序号可复现，不用于安全随机。","Lock the fragments you like and roll a surprise. Seed and turn are reproducible, not security randomness."),"fields":fields,"groups":groups,"blocks":blocks,"errors":errors,"effects":effects,"artifacts":artifacts})).map_err(|_|"Cannot encode story view".into())
}

fn action_index(action: &str, prefix: &str, length: usize) -> Result<usize, String> {
    let index = action[prefix.len()..]
        .parse::<usize>()
        .map_err(|_| "Invalid story item index")?;
    if index >= length || action != format!("{prefix}{index}") {
        return Err("Story item no longer exists".into());
    }
    Ok(index)
}

#[cfg(target_arch = "wasm32")]
use extism_pdk::*;
#[cfg(target_arch = "wasm32")]
#[plugin_fn]
pub fn story(input: String) -> FnResult<String> {
    process(&input).map_err(|error| Error::msg(error).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn initial() -> Value {
        json!({"version":2,"action":null,"state":"","fields":{},"locale":"en"})
    }
    fn run(input: &Value) -> Value {
        serde_json::from_str(&process(&input.to_string()).unwrap()).unwrap()
    }
    fn next(view: &Value, action: &str) -> Value {
        let fields: BTreeMap<String, String> = view["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| {
                (
                    field["id"].as_str().unwrap().to_owned(),
                    field["value"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        json!({"version":2,"action":action,"state":view["state"],"fields":fields,"locale":"en"})
    }
    fn host(request: &mut Value, revision: i32, values: Value) {
        request["host"] =
            json!({"permissions":["storage"],"storage":{"revision":revision,"values":values}});
    }
    #[test]
    fn seeded_dice_are_reproducible_and_lexicon_indices_bounded() {
        assert_eq!(
            throw("repeat", "fantasy", 23),
            throw("repeat", "fantasy", 23)
        );
        assert_ne!(
            throw("repeat", "fantasy", 23),
            throw("repeat", "fantasy", 24)
        );
        for turn in 1..1000 {
            assert!(throw("种子", "cozy", turn).iter().all(|n| *n < 8));
        }
    }
    #[test]
    fn locking_preserves_choices_and_rolling_preserves_the_draft() {
        let first = run(&initial());
        let mut request = next(&first, "roll");
        request["fields"]["lock-0"] = json!("true");
        request["fields"]["draft"] = json!("A draft I must not lose");
        let result = run(&request);
        let before: State = serde_json::from_str(first["state"].as_str().unwrap()).unwrap();
        let after: State = serde_json::from_str(result["state"].as_str().unwrap()).unwrap();
        assert_eq!(before.roll.dice[0], after.roll.dice[0]);
        assert_eq!(after.draft, "A draft I must not lose");
        assert_eq!(after.history.len(), 1);
    }
    #[test]
    fn no_storage_means_no_fake_persistence_and_export_still_works() {
        let first = run(&initial());
        let result = run(&next(&first, "save"));
        assert!(result["effects"].as_array().unwrap().is_empty());
        let exported = run(&next(&result, "export"));
        assert_eq!(exported["artifacts"][0]["name"], "story-draft.md");
        assert!(
            exported["artifacts"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Seed:")
        );
    }
    #[test]
    fn storage_uses_revision_and_only_receipt_confirms_success() {
        let first = run(&initial());
        let mut request = next(&first, "save");
        host(&mut request, 7, json!({}));
        let pending = run(&request);
        assert_eq!(pending["effects"][0]["expectedRevision"], 7);
        let values = pending["effects"][0]["changes"].clone();
        let mut callback = next(&pending, "host-result");
        host(&mut callback, 8, values.clone());
        callback["results"] =
            json!([{"id":"write-favorites","value":{"revision":8,"values":values},"error":null}]);
        let confirmed = run(&callback);
        assert!(confirmed["effects"].as_array().unwrap().is_empty());
        assert!(
            confirmed["blocks"][1]["text"]
                .as_str()
                .unwrap()
                .contains("Host confirmed")
        );
        let mut reopen = initial();
        host(&mut reopen, 8, values.clone());
        let loaded = run(&reopen);
        assert_eq!(loaded["blocks"][2]["items"][2]["value"], "1/8");
        let mut restore = next(&loaded, "favorite-0");
        host(&mut restore, 8, values.clone());
        let restored: State =
            serde_json::from_str(run(&restore)["state"].as_str().unwrap()).unwrap();
        let original: State = serde_json::from_str(first["state"].as_str().unwrap()).unwrap();
        assert_eq!(restored.draft, original.draft);
        let mut remove = next(&loaded, "remove-0");
        host(&mut remove, 8, values);
        let removed = run(&remove);
        assert_eq!(removed["effects"][0]["expectedRevision"], 8);
        assert_eq!(
            removed["effects"][0]["changes"][STORAGE_KEY]["items"],
            json!([])
        );
    }
    #[test]
    fn conflict_is_visible_without_replaying_the_effect() {
        let first = run(&initial());
        let mut request = next(&first, "save");
        host(&mut request, 0, json!({}));
        let pending = run(&request);
        let mut callback = next(&pending, "host-result");
        host(&mut callback, 1, json!({}));
        callback["results"] = json!([{"id":"write-favorites","value":null,"error":{"code":"CONFLICT","message":"another window wrote","retryable":false}}]);
        let failed = run(&callback);
        assert!(failed["effects"].as_array().unwrap().is_empty());
        assert!(
            failed["blocks"][1]["text"]
                .as_str()
                .unwrap()
                .contains("CONFLICT")
        );
    }
    #[test]
    fn malformed_actions_state_fields_and_future_storage_are_rejected() {
        let first = run(&initial());
        for action in [
            "unknown",
            "history-900",
            "history-00",
            "favorite-0",
            "remove-0",
            "host-result",
        ] {
            assert!(
                process(&next(&first, action).to_string()).is_err(),
                "{action}"
            );
        }
        let mut request = next(&first, "roll");
        request["fields"]["lock-5"] = json!("true");
        assert!(process(&request.to_string()).is_err());
        let mut request = next(&first, "roll");
        request["fields"]["draft"] = json!("a".repeat(DRAFT_LIMIT + 1));
        assert!(process(&request.to_string()).is_err());
        let mut request = initial();
        request["state"] = json!("{\"version\":999}");
        assert!(process(&request.to_string()).is_err());
        let mut request = initial();
        host(
            &mut request,
            0,
            json!({STORAGE_KEY:{"version":2,"items":[]}}),
        );
        assert!(process(&request.to_string()).is_err());
    }
    #[test]
    fn invalid_turn_reports_field_error_and_history_remains_bounded() {
        let mut view = run(&initial());
        let mut invalid = next(&view, "roll");
        invalid["fields"]["turn"] = json!("0");
        let response = run(&invalid);
        assert!(response["errors"]["turn"].is_string());
        for _ in 0..20 {
            view = run(&next(&view, "roll"));
        }
        let state: State = serde_json::from_str(view["state"].as_str().unwrap()).unwrap();
        assert_eq!(state.history.len(), MAX_HISTORY);
    }
    #[test]
    fn language_and_fixed_export_basename_survive_user_text() {
        let mut request = initial();
        request["locale"] = json!("zh-CN");
        let first = run(&request);
        assert_eq!(first["display"], "故事骰子");
        let mut request = next(&first, "export");
        request["fields"]["title"] = json!("../CON");
        let exported = run(&request);
        assert_eq!(exported["artifacts"][0]["name"], "story-draft.md");
        request["fields"]["title"] = json!("single\nline");
        assert!(process(&request.to_string()).is_err());
    }
    #[test]
    fn long_unicode_favorite_titles_fit_host_action_labels() {
        let first = run(&initial());
        let original: State = serde_json::from_str(first["state"].as_str().unwrap()).unwrap();
        let mut request = initial();
        host(
            &mut request,
            2,
            json!({STORAGE_KEY:{"version":1,"items":[{"title":"界".repeat(40),"draft":"draft","roll":original.roll}]}}),
        );
        let response = run(&request);
        assert!(
            response["groups"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|group| group["actions"].as_array().unwrap())
                .all(|button| button["label"].as_str().unwrap().len() <= 120)
        );
    }
    #[test]
    fn stale_favorite_buttons_cannot_load_or_remove_another_windows_item() {
        let first = run(&initial());
        let state: State = serde_json::from_str(first["state"].as_str().unwrap()).unwrap();
        let values = json!({STORAGE_KEY:{"version":1,"items":[{"title":"Original favorite","draft":"original","roll":state.roll}]}});
        let mut request = initial();
        host(&mut request, 1, values.clone());
        let old = run(&request);
        let replacement = json!({STORAGE_KEY:{"version":1,"items":[{"title":"Other window favorite","draft":"other window","roll":state.roll}]}});
        for action in ["remove-0", "favorite-0"] {
            let mut request = next(&old, action);
            request["fields"]["draft"] = json!("Keep my unsaved draft");
            host(&mut request, 2, replacement.clone());
            let response = run(&request);
            let state: State = serde_json::from_str(response["state"].as_str().unwrap()).unwrap();
            assert_eq!(state.draft, "Keep my unsaved draft");
            assert_eq!(state.favorites_revision, Some(2));
            assert!(state.status.contains("another window"));
            assert!(response["effects"].as_array().unwrap().is_empty());
        }
    }
}
