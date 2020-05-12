use std::collections::{HashMap, HashSet};

use rand::{thread_rng, Rng, rngs::ThreadRng, distributions::Uniform};

const PLAYERS: [&str; 10] = ["#", "@", "%", "$", "*", "z", "+", "=", "?", "Q"];

const BOARD_SIZE: Vector2 = Vector2(64, 24);

#[derive(Hash)]
pub struct Vector2(pub i64, pub i64);

impl Vector2 {
    pub fn translate(&mut self, v: &Vector2, wrap_around: &Vector2) {
        self.0 += v.0;
        self.1 += v.1;
        if self.0 < 0 {
            self.0 += wrap_around.0;
        }
        if self.1 < 0 {
            self.1 += wrap_around.1;
        }
        if self.0 >= wrap_around.0 {
            self.0 %= wrap_around.0;
        }
        if self.1 >= wrap_around.1 {
            self.1 %= wrap_around.1;
        }
    }
}

impl Clone for Vector2 {
    fn clone(&self) -> Self {
        Vector2(self.0, self.1)
    }
}

impl PartialEq for Vector2 {
    fn eq(&self, other: &Vector2) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl Eq for Vector2 { }

pub enum Actor {
    Empty,
    Fruit,
    Player(usize),
}

pub struct Player {
    id: u64,
    coords: Vec<Vector2>,
    dir: Vector2,
    score: u64,
    dead: bool,
}

impl Player {
    pub fn new(id: u64, coords: Vec<Vector2>, initial_dir: Vector2) -> Player {
        Player {
            id,
            coords,
            dir: initial_dir,
            score: 0,
            dead: false,
        }
    }
    pub fn set_as_dead(&mut self) {
        self.dead = true;
    }
    pub fn is_dead(&self) -> bool {
        self.dead
    }
    pub fn set_dir(&mut self, dir: Vector2) {
        self.dir = dir;
    }
    pub fn get_id(&self) -> u64 {
        self.id
    }
    pub fn get_score(&self) -> u64 {
        self.score
    }
}

impl Clone for Player {
    fn clone(&self) -> Self {
        Player {
            id: self.id,
            coords: self.coords.clone(),
            dir: self.dir.clone(),
            score: self.score,
            dead: self.dead,
        }
    }
}

pub struct Game {
    pub stage: u64,
    players: Vec<Player>,
    board: Vec<Vec<Actor>>,
    board_size: Vector2,
    rng: ThreadRng,
}
//⬆➡⬇⬅
impl Game {
    pub fn new(pid: &[u64]) -> Game {
        let mut board = Game::empty_board();
        let mut players = Vec::new();
        for p in 0..pid.len() {
            let r = (21 / (pid.len() + 1)) * (p + 1);
            board[r][3] = Actor::Player(p);
            board[r][2] = Actor::Player(p);
            players.push(Player::new(pid[p], vec![Vector2(2, r as i64), Vector2(3, r as i64)], Vector2(1, 0)));
        }
        Game { stage: 0,
            players,
            board,
            board_size: BOARD_SIZE,
            rng: thread_rng(), }
    }
    pub fn empty_board() -> Vec<Vec<Actor>> {
        let mut board: Vec<Vec<Actor>> = Vec::with_capacity(BOARD_SIZE.1 as usize);
        for i in 0..BOARD_SIZE.1 {
            let mut row: Vec<Actor> = Vec::with_capacity(BOARD_SIZE.0 as usize);
            for j in 0..BOARD_SIZE.0 {
                row.push(Actor::Empty);
            }
            board.push(row);
        }
        board
    }
    pub fn as_str(&self) -> String {
        let mut s = String::new();
        for r in &self.board {
            for c in r {
                s += match c {
                    Actor::Empty => "-", //:white_large_square:?
                    Actor::Fruit => "*",
                    Actor::Player(i) => PLAYERS[*i],
                };
            }
            s += "\n";
        }
        s.trim().to_owned()
    }
    pub fn get_player_by_id(&mut self, id: u64) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }
    pub fn place_fruit(&mut self) {
        let mut possible_positions = Vec::new();
        for y in 0..self.board_size.1 {
            for x in 0..self.board_size.0 {
                if let Actor::Empty = self.board[y as usize][x as usize] {
                    possible_positions.push((x, y));
                }
            }
        }
        let f = possible_positions[self.rng.gen_range(0, possible_positions.len())];
        self.board[f.1 as usize][f.0 as usize] = Actor::Fruit;
    }
    pub fn get_rankings(&self) -> Vec<Player> {
        let mut players = self.players.clone();
        players.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        let mut dead = Vec::new();
        let mut alive = Vec::new();
        for p in players {
            match p.is_dead() {
                true => &mut dead,
                false => &mut alive,
            }.push(p);
        }
        alive.extend(dead);
        alive
    }
    pub fn tick(&mut self) -> (String, Option<Vec<u64>>) {
        let new_fruit = self.rng.sample::<f64, _>(Uniform::new(0.0, 1.0));
        if new_fruit < 0.3 {
            self.place_fruit();
        }
        let mut winners: HashSet<u64> = HashSet::new();
        let mut new_positions: HashMap<Vector2, &mut Player> = HashMap::new();
        for (i, ap) in self.players.iter_mut().enumerate() {
            if !ap.is_dead() {
                let mut got_fruit = false;
                let mut c = ap.coords[ap.coords.len()-1].clone();
                c.translate(&ap.dir, &self.board_size);
                match self.board[c.1 as usize][c.0 as usize] {
                    Actor::Empty => {},
                    Actor::Fruit => {
                        ap.score += 1;
                        got_fruit = true;
                    },
                    Actor::Player(i) => {
                        ap.set_as_dead();
                        winners.insert(ap.id);
                    },
                }
                if !got_fruit {
                    let tail = ap.coords.remove(0);
                    self.board[tail.1 as usize][tail.0 as usize] = Actor::Empty;
                }
                self.board[c.1 as usize][c.0 as usize] = Actor::Player(i);
                ap.coords.push(c.clone());
                match new_positions.get_mut(&c) {
                    Some(t) => {
                        t.set_as_dead();
                        ap.set_as_dead();
                        winners.insert(t.id);
                        winners.insert(ap.id);
                    },
                    None => {
                        new_positions.insert(c.clone(), ap);
                    },
                }
            }
        }
        let mut non_dead = Vec::new();
        for p in &self.players {
            if !p.is_dead() {
                non_dead.push(p.id);
            }
        }
        let winners = if non_dead.len() == 0 {
            Some(winners.into_iter().collect())
        } else if non_dead.len() == 1 {
            Some(vec![non_dead[0]])
        } else {
            None
        };
        (self.as_str(), winners)
    }
}