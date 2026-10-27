//! Integer 20 Hz port of the MIT-licensed littensy/slither arena rules.
//! The browser and Wasm independently replay the same bounded input log.
use serde::{Deserialize, Serialize};

pub const MAX_TICKS: u32 = 12_000;
pub const MAX_INPUTS: usize = MAX_TICKS as usize;
const RADIUS: i32 = 102_400;
const MAX_BODY: usize = 160;
const BOT_COUNT: usize = 12;
const FOOD_COUNT: usize = 540;
const MAX_FOOD: usize = 700;
const CELL: i32 = 6_400;
const GRID: usize = 34;
const CELLS: usize = GRID * GRID;
const SPEED: i32 = 240;
const BOOST_SPEED: i32 = 480;
const COS: [i32; 256] = [
    1024, 1024, 1023, 1021, 1019, 1016, 1013, 1009, 1004, 999, 993, 987, 980, 972, 964, 955, 946,
    936, 926, 915, 903, 891, 878, 865, 851, 837, 822, 807, 792, 775, 759, 742, 724, 706, 688, 669,
    650, 630, 610, 590, 569, 548, 526, 505, 483, 460, 438, 415, 392, 369, 345, 321, 297, 273, 249,
    224, 200, 175, 150, 125, 100, 75, 50, 25, 0, -25, -50, -75, -100, -125, -150, -175, -200, -224,
    -249, -273, -297, -321, -345, -369, -392, -415, -438, -460, -483, -505, -526, -548, -569, -590,
    -610, -630, -650, -669, -688, -706, -724, -742, -759, -775, -792, -807, -822, -837, -851, -865,
    -878, -891, -903, -915, -926, -936, -946, -955, -964, -972, -980, -987, -993, -999, -1004,
    -1009, -1013, -1016, -1019, -1021, -1023, -1024, -1024, -1024, -1023, -1021, -1019, -1016,
    -1013, -1009, -1004, -999, -993, -987, -980, -972, -964, -955, -946, -936, -926, -915, -903,
    -891, -878, -865, -851, -837, -822, -807, -792, -775, -759, -742, -724, -706, -688, -669, -650,
    -630, -610, -590, -569, -548, -526, -505, -483, -460, -438, -415, -392, -369, -345, -321, -297,
    -273, -249, -224, -200, -175, -150, -125, -100, -75, -50, -25, 0, 25, 50, 75, 100, 125, 150,
    175, 200, 224, 249, 273, 297, 321, 345, 369, 392, 415, 438, 460, 483, 505, 526, 548, 569, 590,
    610, 630, 650, 669, 688, 706, 724, 742, 759, 775, 792, 807, 822, 837, 851, 865, 878, 891, 903,
    915, 926, 936, 946, 955, 964, 972, 980, 987, 993, 999, 1004, 1009, 1013, 1016, 1019, 1021,
    1023, 1024,
];
const ATAN: [i32; 257] = [
    0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 5, 5, 5,
    5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 10, 10,
    10, 10, 10, 10, 10, 11, 11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13,
    13, 13, 14, 14, 14, 14, 14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 16, 16, 16, 16, 16, 16, 16, 17,
    17, 17, 17, 17, 17, 17, 17, 18, 18, 18, 18, 18, 18, 18, 19, 19, 19, 19, 19, 19, 19, 19, 20, 20,
    20, 20, 20, 20, 20, 20, 21, 21, 21, 21, 21, 21, 21, 21, 21, 22, 22, 22, 22, 22, 22, 22, 22, 23,
    23, 23, 23, 23, 23, 23, 23, 23, 24, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25,
    25, 25, 25, 26, 26, 26, 26, 26, 26, 26, 26, 26, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 28, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 30, 30, 30, 30,
    30, 30, 30, 30, 30, 30, 30, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 31, 32, 32, 32, 32, 32,
    32, 32,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub schema: u8,
    pub seed: u32,
    pub difficulty: Difficulty,
    pub skin: u8,
    pub muted: bool,
    pub volume: u8,
    pub nickname: String,
    pub inputs: Vec<(u8, bool, u16)>,
}
impl Default for Checkpoint {
    fn default() -> Self {
        Self {
            schema: 3,
            seed: 20_261_026,
            difficulty: Difficulty::Normal,
            skin: 0,
            muted: false,
            volume: 65,
            nickname: String::new(),
            inputs: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub score: u32,
    pub max_score: u32,
    pub kills: u32,
    pub foods: u32,
    pub ticks: u32,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snake {
    pub id: usize,
    pub alive: bool,
    pub x: i32,
    pub y: i32,
    pub angle: i32,
    pub target_angle: i32,
    pub score: u32,
    pub best_score: u32,
    pub kills: u32,
    pub foods: u32,
    pub boosting: bool,
    pub radius: i32,
    pub length: usize,
    pub turn: i32,
    pub respawn_at: u32,
    pub body: Vec<(i32, i32)>,
}
impl Snake {
    fn describe(&mut self) {
        self.radius = 400.max(560 * log10(self.score + 600, 300) / 1024);
        self.length =
            (((64 * log10(self.score + 256, 256) + 3072) as i64 * i64::from(self.radius) * 3)
                / (2048 * i64::from(SPEED))) as usize;
        self.length = self.length.clamp(5, MAX_BODY);
        self.turn =
            2.max((360 * 1024 - 100 * log10(self.score + 150, 150)) * 256 / (360 * 20 * 1024));
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Food {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub value: u32,
    pub color: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Grid {
    heads: Vec<i32>,
    next: Vec<i32>,
    x: Vec<i32>,
    y: Vec<i32>,
    id: Vec<usize>,
    food_heads: Vec<i32>,
    food_next: Vec<i32>,
    count: usize,
}
impl Grid {
    fn new() -> Self {
        let nodes = (BOT_COUNT + 1) * MAX_BODY;
        Self {
            heads: vec![-1; CELLS],
            next: vec![-1; nodes],
            x: vec![0; nodes],
            y: vec![0; nodes],
            id: vec![0; nodes],
            food_heads: vec![-1; CELLS],
            food_next: vec![-1; MAX_FOOD],
            count: 0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub tick: u32,
    pub rng: u32,
    pub difficulty: Difficulty,
    pub status: String,
    pub snakes: Vec<Snake>,
    pub foods: Vec<Food>,
    next_food_id: u32,
    grid: Grid,
}

fn sin(angle: i32) -> i32 {
    COS[((angle + 192) & 255) as usize]
}
fn distance2(x: i32, y: i32, a: i32, b: i32) -> i64 {
    i64::from(x - a).pow(2) + i64::from(y - b).pow(2)
}
fn angle_to(x: i32, y: i32) -> i32 {
    let ax = x.abs();
    let ay = y.abs();
    if ax == 0 && ay == 0 {
        return 0;
    }
    let mut angle = if ax >= ay {
        ATAN[(ay * 256 / ax) as usize]
    } else {
        64 - ATAN[(ax * 256 / ay) as usize]
    };
    if x < 0 {
        angle = 128 - angle;
    }
    if y < 0 {
        angle = 256 - angle;
    }
    angle & 255
}
fn log10(numerator: u32, denominator: u32) -> i32 {
    let mut value = u64::from(numerator) * 65_536 / u64::from(denominator);
    let mut exponent: u32 = 0;
    let mut fraction: u32 = 0;
    while value >= 131_072 {
        value /= 2;
        exponent += 1;
    }
    let mut bit = 512;
    while bit > 0 {
        value = value * value / 65_536;
        if value >= 131_072 {
            value /= 2;
            fraction += bit;
        }
        bit >>= 1;
    }
    ((u64::from(exponent * 1024 + fraction) * 315_653) / 1_048_576) as i32
}
fn cell(x: i32, y: i32) -> usize {
    ((x + RADIUS) / CELL).clamp(0, GRID as i32 - 1) as usize
        + ((y + RADIUS) / CELL).clamp(0, GRID as i32 - 1) as usize * GRID
}
impl World {
    pub fn new(seed: u32, difficulty: Difficulty) -> Self {
        let mut world = Self {
            tick: 0,
            rng: seed,
            difficulty,
            status: "running".into(),
            snakes: Vec::with_capacity(BOT_COUNT + 1),
            foods: Vec::with_capacity(MAX_FOOD),
            next_food_id: 0,
            grid: Grid::new(),
        };
        for _ in 0..FOOD_COUNT {
            let (x, y) = world.point(RADIUS - 2400);
            let food = world.food(x, y, 0, None);
            world.foods.push(food);
        }
        for id in 0..=BOT_COUNT {
            let snake = world.make_snake(id);
            world.snakes.push(snake);
        }
        world.build_grid();
        world
    }
    fn random(&mut self) -> u32 {
        let mut value = self.rng;
        value ^= value << 13;
        value ^= value >> 17;
        value ^= value << 5;
        self.rng = value;
        value
    }
    fn point(&mut self, limit: i32) -> (i32, i32) {
        loop {
            let x = (self.random() % (limit * 2 + 1) as u32) as i32 - limit;
            let y = (self.random() % (limit * 2 + 1) as u32) as i32 - limit;
            if distance2(x, y, 0, 0) <= i64::from(limit).pow(2) {
                return (x, y);
            }
        }
    }
    fn food(&mut self, x: i32, y: i32, mut value: u32, color: Option<u32>) -> Food {
        if value == 0 {
            value = (1 + self.random() % 4).min(1 + self.random() % 5);
        }
        let color = color.unwrap_or_else(|| self.random() % 8);
        let food = Food {
            id: self.next_food_id,
            x,
            y,
            value,
            color,
        };
        self.next_food_id += 1;
        food
    }
    fn add_food(&mut self, food: Food) {
        if self.foods.len() < MAX_FOOD {
            self.foods.push(food);
        } else {
            let index = food.id as usize % MAX_FOOD;
            self.foods[index] = food;
        }
    }
    fn make_snake(&mut self, id: usize) -> Snake {
        let (mut x, mut y, mut angle, mut score) = (0, 0, 0, 10);
        if id > 0 {
            loop {
                (x, y) = self.point(RADIUS - 12_000);
                if distance2(x, y, 0, 0) >= 20_000_i64.pow(2) {
                    break;
                }
            }
            angle = angle_to(-x, -y);
            score = 25 + self.random() % if id == 1 { 800 } else { 280 };
        }
        let mut snake = Snake {
            id,
            alive: true,
            x,
            y,
            angle,
            target_angle: angle,
            score,
            best_score: score,
            kills: 0,
            foods: 0,
            boosting: false,
            radius: 400,
            length: 5,
            turn: 12,
            respawn_at: 0,
            body: Vec::with_capacity(MAX_BODY),
        };
        snake.describe();
        for index in 0..snake.length as i32 {
            snake.body.push((
                x - COS[angle as usize] * SPEED * index / 1024,
                y - sin(angle) * SPEED * index / 1024,
            ));
        }
        snake
    }
    fn build_grid(&mut self) {
        let grid = &mut self.grid;
        grid.heads.fill(-1);
        grid.count = 0;
        for snake in &self.snakes {
            if !snake.alive {
                continue;
            }
            for index in (2..snake.body.len()).step_by(2) {
                let (x, y) = snake.body[index];
                let bucket = cell(x, y);
                let node = grid.count;
                grid.count += 1;
                grid.x[node] = x;
                grid.y[node] = y;
                grid.id[node] = snake.id;
                grid.next[node] = grid.heads[bucket];
                grid.heads[bucket] = node as i32;
            }
        }
        grid.food_heads.fill(-1);
        for (index, food) in self.foods.iter().enumerate() {
            let bucket = cell(food.x, food.y);
            grid.food_next[index] = grid.food_heads[bucket];
            grid.food_heads[bucket] = index as i32;
        }
    }
    fn danger(&self, x: i32, y: i32, id: usize, padding: i32) -> Option<usize> {
        let grid = &self.grid;
        let bucket = cell(x, y) as i32;
        let cx = bucket % GRID as i32;
        let cy = bucket / GRID as i32;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if !(0..GRID as i32).contains(&(cx + dx)) || !(0..GRID as i32).contains(&(cy + dy))
                {
                    continue;
                }
                let mut node = grid.heads[(bucket + dx + dy * GRID as i32) as usize];
                while node != -1 {
                    let index = node as usize;
                    let other = &self.snakes[grid.id[index]];
                    let radius = (padding + other.radius) * 8 / 10;
                    if other.id != id
                        && other.alive
                        && distance2(x, y, grid.x[index], grid.y[index]) < i64::from(radius).pow(2)
                    {
                        return Some(other.id);
                    }
                    node = grid.next[index];
                }
            }
        }
        None
    }
    fn bot_input(&mut self, id: usize) -> (i32, bool) {
        if (self.tick + id as u32).is_multiple_of(4) {
            let (x, y, radius, angle) = {
                let snake = &self.snakes[id];
                (snake.x, snake.y, snake.radius, snake.angle)
            };
            let (mut tx, mut ty) = (0, 0);
            if distance2(x, y, 0, 0) < i64::from(RADIUS - 11_000).pow(2) {
                let mut best = i64::MAX;
                for _ in 0..18 {
                    let index = self.random() as usize % self.foods.len();
                    let item = &self.foods[index];
                    let weight = distance2(x, y, item.x, item.y) / i64::from(item.value.min(6));
                    if weight < best {
                        best = weight;
                        tx = item.x;
                        ty = item.y;
                    }
                }
            }
            let mut target_angle = angle_to(tx - x, ty - y);
            let look_x = x + COS[target_angle as usize] * 2800 / 1024;
            let look_y = y + sin(target_angle) * 2800 / 1024;
            let padding = radius
                + if self.difficulty == Difficulty::Hard {
                    1800
                } else {
                    900
                };
            if self.danger(look_x, look_y, id, padding).is_some() {
                target_angle = (angle + if self.random() & 1 == 1 { 48 } else { -48 }) & 255;
            }
            self.snakes[id].target_angle = target_angle;
        }
        let chance = match self.difficulty {
            Difficulty::Easy => 65,
            Difficulty::Normal => 40,
            Difficulty::Hard => 22,
        };
        let snake = &self.snakes[id];
        (
            snake.target_angle,
            snake.score > 55 && (self.tick + id as u32 * 13) % chance < 8,
        )
    }
    fn move_snake(&mut self, id: usize, angle: i32, boost: bool) {
        let boosting = boost && self.snakes[id].score > 10;
        self.snakes[id].boosting = boosting;
        if boosting && self.tick.is_multiple_of(3) {
            let max = (3 + (self.snakes[id].score + 500) / 1000).clamp(2, 10);
            let cost = 1 + self.random() % max;
            let snake = &mut self.snakes[id];
            snake.score = snake.score.saturating_sub(cost).max(10);
            let (x, y) = snake.body[snake.body.len() - 1];
            snake.describe();
            let food = self.food(x, y, cost, Some(id as u32 % 8));
            self.add_food(food);
        }
        let snake = &mut self.snakes[id];
        let delta = ((angle - snake.angle + 384) & 255) - 128;
        snake.angle = (snake.angle + delta.clamp(-snake.turn, snake.turn)) & 255;
        let speed = if boosting { BOOST_SPEED } else { SPEED };
        let dx = COS[snake.angle as usize] * speed / 1024;
        let dy = sin(snake.angle) * speed / 1024;
        if boosting {
            snake.body.insert(0, (snake.x + dx / 2, snake.y + dy / 2));
        }
        snake.x += dx;
        snake.y += dy;
        snake.body.insert(0, (snake.x, snake.y));
        snake.body.truncate(snake.length);
    }
    fn kill(&mut self, id: usize, killer: i32) {
        let snake = &mut self.snakes[id];
        if !snake.alive {
            return;
        }
        snake.alive = false;
        snake.boosting = false;
        snake.respawn_at = self.tick + 60;
        let count = snake.body.len().div_ceil(3);
        let value = (snake.score / count.max(1) as u32).clamp(1, 50);
        let body = snake.body.clone();
        if killer >= 0 && killer as usize != id {
            self.snakes[killer as usize].kills += 1;
        }
        for &(x, y) in body.iter().step_by(3) {
            let x = x + (self.random() % 401) as i32 - 200;
            let y = y + (self.random() % 401) as i32 - 200;
            let food = self.food(x, y, value, Some(id as u32 % 8));
            self.add_food(food);
        }
    }
    fn eat(&mut self, id: usize) {
        let (x, y) = (self.snakes[id].x, self.snakes[id].y);
        let bucket = cell(x, y) as i32;
        let cx = bucket % GRID as i32;
        let cy = bucket / GRID as i32;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if !(0..GRID as i32).contains(&(cx + dx)) || !(0..GRID as i32).contains(&(cy + dy))
                {
                    continue;
                }
                let mut index = self.grid.food_heads[(bucket + dx + dy * GRID as i32) as usize];
                while index != -1 {
                    let item = &self.foods[index as usize];
                    let radius = self.snakes[id].radius + 420 + item.value as i32 * 20;
                    if distance2(x, y, item.x, item.y) < i64::from(radius).pow(2) {
                        let snake = &mut self.snakes[id];
                        snake.score = (snake.score + item.value).min(100_000);
                        snake.best_score = snake.best_score.max(snake.score);
                        snake.foods += 1;
                        snake.describe();
                        let (fx, fy) = self.point(RADIUS - 2400);
                        let food = self.food(fx, fy, 0, None);
                        self.foods[index as usize] = food;
                    }
                    index = self.grid.food_next[index as usize];
                }
            }
        }
    }
    pub fn step(&mut self, angle: u8, boost: bool) -> Result<(), String> {
        if self.status != "running" {
            return Err("Unreachable arena history / 竞技操作历史不可达".into());
        }
        self.tick += 1;
        for id in 0..self.snakes.len() {
            if !self.snakes[id].alive && id > 0 && self.tick >= self.snakes[id].respawn_at {
                self.snakes[id] = self.make_snake(id);
            }
            if self.snakes[id].alive {
                let (angle, boost) = if id > 0 {
                    self.bot_input(id)
                } else {
                    (i32::from(angle), boost)
                };
                self.move_snake(id, angle, boost);
            }
        }
        self.build_grid();
        let mut killers = [-2; BOT_COUNT + 1];
        for snake in &self.snakes {
            if !snake.alive {
                continue;
            }
            if distance2(snake.x, snake.y, 0, 0) > i64::from(RADIUS - snake.radius).pow(2) {
                killers[snake.id] = -1;
                continue;
            }
            if let Some(id) = self.danger(snake.x, snake.y, snake.id, snake.radius) {
                killers[snake.id] = id as i32;
            }
        }
        for left in 0..self.snakes.len() {
            for right in left + 1..self.snakes.len() {
                let a = &self.snakes[left];
                let b = &self.snakes[right];
                let radius = (a.radius + b.radius) * 8 / 10;
                if a.alive && b.alive && distance2(a.x, a.y, b.x, b.y) < i64::from(radius).pow(2) {
                    if a.score <= b.score {
                        killers[left] = right as i32;
                    }
                    if b.score <= a.score {
                        killers[right] = left as i32;
                    }
                }
            }
        }
        for (id, killer) in killers.iter().enumerate() {
            if *killer != -2 {
                self.kill(id, *killer);
            }
        }
        for id in 0..self.snakes.len() {
            if self.snakes[id].alive {
                self.eat(id);
            }
        }
        self.status = if !self.snakes[0].alive {
            "dead"
        } else if self.tick >= MAX_TICKS {
            "finished"
        } else {
            "running"
        }
        .into();
        Ok(())
    }
}

pub fn validate_checkpoint(checkpoint: &Checkpoint) -> Result<(), String> {
    if checkpoint.schema != 3
        || checkpoint.seed == 0
        || checkpoint.skin > 14
        || checkpoint.volume > 100
        || checkpoint.nickname.chars().count() > 24
        || checkpoint.nickname.chars().any(char::is_control)
        || checkpoint.inputs.len() > MAX_INPUTS
        || checkpoint
            .inputs
            .iter()
            .any(|(_, _, ticks)| *ticks == 0 || u32::from(*ticks) > MAX_TICKS)
        || checkpoint
            .inputs
            .iter()
            .map(|(_, _, ticks)| u32::from(*ticks))
            .sum::<u32>()
            > MAX_TICKS
    {
        return Err("Invalid or future arena checkpoint / 进度损坏或版本较新，已保留原件".into());
    }
    Ok(())
}
pub fn replay(checkpoint: &Checkpoint) -> Result<World, String> {
    validate_checkpoint(checkpoint)?;
    let mut world = World::new(checkpoint.seed, checkpoint.difficulty);
    for (angle, boost, ticks) in &checkpoint.inputs {
        for _ in 0..*ticks {
            world.step(*angle, *boost)?;
        }
    }
    Ok(world)
}
pub fn summary(world: &World) -> Summary {
    let snake = &world.snakes[0];
    Summary {
        score: snake.score,
        max_score: snake.best_score,
        kills: snake.kills,
        foods: snake.foods,
        ticks: world.tick,
        status: world.status.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both language implementations use this full-world fingerprint, so a
    // matching score alone cannot hide AI, food, collision or RNG divergence.
    fn fingerprint(world: &World) -> u32 {
        fn number(hash: &mut u32, value: u32) {
            for byte in value.to_le_bytes() {
                *hash = (*hash ^ u32::from(byte)).wrapping_mul(16_777_619);
            }
        }
        let mut hash = 2_166_136_261;
        for value in [world.tick, world.rng, world.next_food_id] {
            number(&mut hash, value);
        }
        for snake in &world.snakes {
            for value in [
                snake.id as u32,
                u32::from(snake.alive),
                snake.x as u32,
                snake.y as u32,
                snake.angle as u32,
                snake.target_angle as u32,
                snake.score,
                snake.best_score,
                snake.kills,
                snake.foods,
                u32::from(snake.boosting),
                snake.radius as u32,
                snake.length as u32,
                snake.turn as u32,
                snake.respawn_at,
                snake.body.len() as u32,
            ] {
                number(&mut hash, value);
            }
            for &(x, y) in &snake.body {
                number(&mut hash, x as u32);
                number(&mut hash, y as u32);
            }
        }
        number(&mut hash, world.foods.len() as u32);
        for food in &world.foods {
            for value in [
                food.id,
                food.x as u32,
                food.y as u32,
                food.value,
                food.color,
            ] {
                number(&mut hash, value);
            }
        }
        hash
    }

    #[test]
    fn browser_and_wasm_worlds_match_after_boosting_bot_deaths_and_respawns() {
        let short = Checkpoint {
            inputs: vec![(13, false, 8), (81, false, 15), (130, true, 12)],
            ..Checkpoint::default()
        };
        assert_eq!(fingerprint(&replay(&short).unwrap()), 0x488d_3971);
        let circle = Checkpoint {
            seed: 0x1234_5678,
            difficulty: Difficulty::Hard,
            inputs: (0..1200)
                .map(|tick| (((tick * 3) & 255) as u8, tick % 100 < 20, 1))
                .collect(),
            ..Checkpoint::default()
        };
        assert_eq!(fingerprint(&replay(&circle).unwrap()), 0xfa22_d378);
        let wall = Checkpoint {
            seed: 9,
            difficulty: Difficulty::Easy,
            inputs: vec![(0, false, 426)],
            ..Checkpoint::default()
        };
        let world = replay(&wall).unwrap();
        assert_eq!(world.status, "dead");
        assert_eq!(fingerprint(&world), 0x7715_faab);
    }

    #[test]
    fn ten_minute_round_is_bounded_and_matches_browser_replay() {
        let checkpoint = Checkpoint {
            seed: 1,
            difficulty: Difficulty::Easy,
            inputs: (0..1200)
                .map(|segment| (((segment * 10) & 255) as u8, false, 10))
                .collect(),
            ..Checkpoint::default()
        };
        let world = replay(&checkpoint).unwrap();
        assert_eq!(world.status, "finished");
        assert_eq!(world.tick, MAX_TICKS);
        assert_eq!(world.snakes[0].score, 83);
        assert_eq!(world.snakes[0].foods, 35);
        assert_eq!(fingerprint(&world), 0xa642_5d75);
        assert!(world.foods.len() <= MAX_FOOD);
        assert!(
            world
                .snakes
                .iter()
                .all(|snake| snake.body.len() <= MAX_BODY)
        );
    }

    #[test]
    fn angles_cover_all_quadrants_without_grid_restriction() {
        assert_eq!(angle_to(1, 0), 0);
        assert_eq!(angle_to(0, 1), 64);
        assert_eq!(angle_to(-1, 0), 128);
        assert_eq!(angle_to(0, -1), 192);
        assert_eq!(angle_to(1, 1), 32);
        let mut world = World::new(20_261_026, Difficulty::Normal);
        for _ in 0..8 {
            world.step(13, false).unwrap();
        }
        assert!(world.snakes[0].x > 0 && world.snakes[0].y > 0);
        assert_eq!(world.snakes[0].angle, 13);
    }

    #[test]
    fn rejects_future_and_unbounded_checkpoints() {
        let mut checkpoint = Checkpoint {
            schema: 4,
            ..Checkpoint::default()
        };
        assert!(replay(&checkpoint).is_err());
        checkpoint.schema = 3;
        checkpoint.inputs = vec![(0, false, MAX_TICKS as u16), (0, false, 1)];
        assert!(replay(&checkpoint).is_err());
        checkpoint.inputs = vec![(0, false, 0)];
        assert!(replay(&checkpoint).is_err());
        checkpoint.inputs.clear();
        checkpoint.nickname = "\u{0000}".into();
        assert!(replay(&checkpoint).is_err());
        let mut value = serde_json::to_value(Checkpoint::default()).unwrap();
        value["score"] = 999.into();
        assert!(serde_json::from_value::<Checkpoint>(value).is_err());
    }

    #[test]
    fn replay_is_seeded_and_continuous() {
        let checkpoint = Checkpoint {
            inputs: vec![(13, false, 8), (81, false, 15), (130, true, 12)],
            ..Checkpoint::default()
        };
        let a = replay(&checkpoint).unwrap();
        let b = replay(&checkpoint).unwrap();
        assert_eq!(a, b);
        let mut other = checkpoint;
        other.seed += 1;
        assert_ne!(a.foods, replay(&other).unwrap().foods);
    }

    #[test]
    fn own_body_can_be_crossed_and_other_bodies_are_lethal() {
        let mut world = World::new(12, Difficulty::Easy);
        for snake in &mut world.snakes[1..] {
            snake.alive = false;
            snake.respawn_at = MAX_TICKS + 1;
        }
        world.snakes[0].body = vec![(240, 0); 20];
        world.step(0, false).unwrap();
        assert!(world.snakes[0].alive);
        let mut other = world.make_snake(1);
        other.x = 8000;
        other.y = 8000;
        other.body = vec![(480, 0); 20];
        world.snakes[1] = other;
        world.step(0, false).unwrap();
        assert!(!world.snakes[0].alive);
        assert!(world.step(0, false).is_err());
    }

    #[test]
    fn boosting_consumes_mass_and_drops_food() {
        let mut world = World::new(8, Difficulty::Easy);
        world.snakes[0].score = 100;
        world.snakes[0].best_score = 100;
        let before = world.foods.len();
        for _ in 0..3 {
            world.step(0, true).unwrap();
        }
        assert!(world.snakes[0].score < 100);
        assert_eq!(world.snakes[0].best_score, 100);
        assert!(world.foods.len() > before);
        assert!(world.snakes[0].x >= 1400);
    }
}
