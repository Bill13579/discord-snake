use std::process;

use std::sync::{Arc, Mutex, mpsc, mpsc::{Sender}};

use std::{thread, thread::sleep};

use std::time::Duration;

use std::collections::{HashMap, HashSet};

use regex::Regex;

use serenity::{
    model::{channel::Message, channel::Reaction, channel::ReactionType, guild::Guild, id::{ChannelId, UserId}},
    prelude::*,
};

use discord_snake::{Game, Vector2, Player, UP, RIGHT, DOWN, LEFT, CANCEL, POINTS_PER_KILL};

const SNAKE_CMD: &str = r"^::(snake|solo) *(.*?) *$";
const HELP_CMD: &str = r"^::help *$";

const HELP: &str = "**Welcome to the wonderful world of Discord Snake!**

> **Commands**

Normal (multiplayer)
`::snake @you @someoneElse @up-to-5-people! @you-get-the-idea`

Solo
`::solo`

Help
`::help` (I think that's intuitive enough)";

const GAME_OVER: &str = r"   _____                         ____
 / ____|                       / __ \
| |  __  __ _ _ __ ___   ___  | |  | |_   _____ _ __
| | |_ |/ _` | '_ ` _ \ / _ \ | |  | \ \ / / _ \ '__|
| |__| | (_| | | | | | |  __/ | |__| |\ V /  __/ |
 \_____|\__,_|_| |_| |_|\___|  \____/  \_/ \___|_|";

const MAX_PLAYERS: usize = 5;

fn send(ctx: &Context, c: &ChannelId, msg: &str) -> Message {
    match c.say(ctx.http.clone(), msg) {
        Err(why) => {
            println!("error sending message: {:?}", why);
            process::exit(1);
        },
        Ok(m) => m,
    }
}

fn stylize_ranking(ranking: &Vec<Player>) -> String {
    let mut r = String::from(format!("**Ranking** (`fruit = 1 | kill = {}`)\n", POINTS_PER_KILL));
    for (p, i) in ranking.iter().zip(1..ranking.len()+1) {
        let i_str = i.to_string();
        r.push_str(&format!("    {}: <@{}>, {} points | {} kills, {}\n", match i {
            1 => ":first_place:",
            2 => ":second_place:",
            3 => ":third_place:",
            _ => i_str.as_str(),
        }, p.get_id(), p.get_score(), p.get_kills(), if p.is_dead() {
            ":x: dead"
        } else {
            ":white_check_mark: alive"
        }));
    }
    r
}

pub struct Handler {
    sessions: Arc<Mutex<HashMap<ChannelId, Sender<(u64, Vector2)>>>>,
}

impl Handler {
    pub fn new() -> Handler {
        Handler {
            sessions: Arc::new(Mutex::new(HashMap::new()))
        }
    }
    fn react(&self, ctx: Context, react: Reaction) {
        let mut s = self.sessions.lock().unwrap();
        if let Some(r) = s.get(&react.channel_id) {
            if let ReactionType::Unicode(em) = react.emoji {
                let mut unknown = false;
                let v = match em.as_str() {
                    "⬆" => UP,
                    "➡" => RIGHT,
                    "⬇" => DOWN,
                    "⬅" => LEFT,
                    "❌" => CANCEL,
                    _ => return,
                };
                r.send((react.user_id.0, v));
            }
        }
    }
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        if let Some(c) = Regex::new(SNAKE_CMD).expect("invalid regex").captures(&msg.content) {
            let mut hm = self.sessions.lock().unwrap();
            if hm.contains_key(&msg.channel_id) {
                send(&ctx, &msg.channel_id, &format!(":x: Round already in progress in channel <#{}>", &msg.channel_id));
                return;
            }
            let mut game_mode = c.get(1).unwrap().as_str().to_owned();
            let mut userids: HashSet<u64> = HashSet::new();
            let mut n_of_users = 0;
            for v in Regex::new(r"\s*?<@!?([&]?)([0-9]*)>\s*?").expect("invalid regex").captures_iter(c.get(2).unwrap().as_str()) {
                if v.get(1).unwrap().as_str().trim() != "" {
                    send(&ctx, &msg.channel_id, ":x: A role can't play snake");
                    return;
                }
                let uid = v.get(2).unwrap().as_str().parse();
                if let Err(e) = uid {
                    send(&ctx, &msg.channel_id, ":x: Invalid mention");
                    return;
                }
                let uid: u64 = uid.unwrap();
                userids.insert(uid);
                n_of_users += 1;
            }
            if userids.len() != n_of_users {
                send(&ctx, &msg.channel_id, ":x: Repeating users");
                return;
            }
            let mut userids: Vec<u64> = userids.into_iter().collect();
            if game_mode == "solo" {
                userids.clear();
                userids.push(msg.author.id.0);
            } else if game_mode == "snake" {
                if userids.len() == 0 || userids.len() == 1 {
                    send(&ctx, &msg.channel_id, ":x: Please enter at least 2 users");
                    return;
                }
                if userids.len() > MAX_PLAYERS {
                    send(&ctx, &msg.channel_id, &format!(":x: Play is currently limited to {} users", MAX_PLAYERS));
                    return;
                }
            }
            let (tx, rx) = mpsc::channel::<(u64, Vector2)>();
            hm.insert(msg.channel_id.clone(), tx);
            let mut hm = self.sessions.clone();
            thread::spawn(move || {
                let mut g = Game::new(game_mode, &userids);
                let mut b = g.as_str();
                let mut m = send(&ctx, &msg.channel_id, &b);
                m.react(&ctx, ReactionType::Unicode(String::from("⬆")));
                m.react(&ctx, ReactionType::Unicode(String::from("➡")));
                m.react(&ctx, ReactionType::Unicode(String::from("⬇")));
                m.react(&ctx, ReactionType::Unicode(String::from("⬅")));
                m.react(&ctx, ReactionType::Unicode(String::from("❌")));
                g.stage = m.id.0;
                loop {
                    sleep(Duration::from_secs_f32(1.001));
                    while let Ok(m) = rx.try_recv() {
                        if let Some(p) = g.get_player_by_id(m.0) {
                            if m.1 == CANCEL {
                                p.set_as_dead();
                            } else {
                                if !p.is_dead() {
                                    p.set_dir(m.1);
                                }
                            }
                        }
                    }
                    let (gb, w) = g.tick();
                    match w {
                        None => {
                            m.edit(&ctx, |m| m.content(format!("{}\n```\n{}\n```", stylize_ranking(&g.get_rankings()), gb)));
                        },
                        Some(a) => {
                            let usernames: Vec<String> = a.iter().map(|u| UserId(*u).to_user(&ctx).unwrap().name).collect();
                            let usernames = usernames.join(", ");
                            m.edit(&ctx, |m| m.content(format!("```\n{}\n```\nCongrats!\n\nLasted the longest: {}\n\n{}", GAME_OVER, usernames, stylize_ranking(&g.get_rankings()))));
                            let mut hm = hm.lock().unwrap();
                            hm.remove(&msg.channel_id.clone());
                            break;
                        },
                    }
                }
            });
        } else if let Some(c) = Regex::new(HELP_CMD).expect("invalid regex").captures(&msg.content) {
            send(&ctx, &msg.channel_id, HELP);
        }
    }
    fn reaction_add(&self, ctx: Context, react: Reaction) {
        self.react(ctx, react);
    }
    fn reaction_remove(&self, ctx: Context, react: Reaction) {
        self.react(ctx, react);
    }
    fn guild_create(&self, ctx: Context, guild: Guild, _is_new: bool) {
        if let Some(e) = guild.default_channel(ctx.cache.read().user.id) {
            send(&ctx, &e.read().id, HELP);
        }
    }
}
