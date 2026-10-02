//! Drives a match: sets, ticks, steps, replay assembly.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::bot::TeamSpec;
use crate::bot::registry::{BotHandle, BotLookupError, BotRegistry};
use crate::error::{BotFailure, EngineError};
use crate::game::{DynGame, GameFactory, GameRegistry, GameStatus, SetSetup};
use crate::ids::{BotId, TeamId};
use crate::replay::{MatchConfig, Replay, SetReplay, SetResult, Step, Tick, Usage, write_replay};
use crate::rng::set_seed;
use crate::step::{StepCtx, StepResult};
use crate::summary::{Summary, summarize, write_summary};

/// Called with each set as it ends; an `Err` aborts the match.
pub type SetHook = Box<dyn FnMut(&SetReplay) -> Result<(), EngineError> + Send>;

pub struct MatchSpec {
    match_id: String,
    config: MatchConfig,
    teams: Vec<TeamSpec>,
    replay_path: Option<PathBuf>,
    summary_path: Option<PathBuf>,
    echo_bot_output: bool,
    on_set: Option<SetHook>,
}

impl MatchSpec {
    pub fn new(match_id: impl Into<String>, config: MatchConfig, teams: Vec<TeamSpec>) -> Self {
        Self {
            match_id: match_id.into(),
            config,
            teams,
            replay_path: None,
            summary_path: None,
            echo_bot_output: false,
            on_set: None,
        }
    }

    /// Write the replay here when the match ends.
    pub fn replay_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.replay_path = Some(path.into());
        self
    }

    /// Write the summary here when the match ends.
    pub fn summary_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.summary_path = Some(path.into());
        self
    }

    /// Echo captured bot output to stderr as it happens.
    pub fn echo_bot_output(mut self, echo: bool) -> Self {
        self.echo_bot_output = echo;
        self
    }

    /// Run `hook` after each set, before the next one starts.
    pub fn on_set(mut self, hook: SetHook) -> Self {
        self.on_set = Some(hook);
        self
    }
}

pub struct MatchReport {
    pub replay: Replay,
    pub summary: Summary,
}

impl MatchReport {
    pub fn new(replay: Replay, summary: Summary) -> Self {
        Self { replay, summary }
    }
}

pub struct MatchRunner<'r> {
    factory: &'r GameFactory,
    match_id: String,
    config: MatchConfig,
    bots: BotRegistry,
    replay_path: Option<PathBuf>,
    summary_path: Option<PathBuf>,
    echo_bot_output: bool,
    on_set: Option<SetHook>,
}

impl<'r> MatchRunner<'r> {
    /// Bots are created when the game first schedules them, so team code that fails to
    /// load shows up as a bot failure at that step, not an error here.
    pub fn new(registry: &'r GameRegistry, spec: MatchSpec) -> Result<Self, EngineError> {
        let mut config = spec.config;
        if config.sets == 0 {
            return Err(EngineError::Config("sets must be at least 1".into()));
        }
        if config.max_ticks == 0 {
            return Err(EngineError::Config("max_ticks must be at least 1".into()));
        }
        if spec.teams.is_empty() {
            return Err(EngineError::Config("at least one team is required".into()));
        }
        let teams = spec.teams.len() as u32;
        if config.teams == 0 {
            config.teams = teams;
        } else if config.teams != teams {
            return Err(EngineError::Config(format!(
                "config declares {} teams but {} team specs were given",
                config.teams, teams
            )));
        }
        let factory = registry.get(&config.game)?;
        let bots = BotRegistry::new(spec.teams, config.limits);
        Ok(Self {
            factory,
            match_id: spec.match_id,
            config,
            bots,
            replay_path: spec.replay_path,
            summary_path: spec.summary_path,
            echo_bot_output: spec.echo_bot_output,
            on_set: spec.on_set,
        })
    }

    pub fn run(mut self) -> Result<MatchReport, EngineError> {
        let mut sets = Vec::with_capacity(self.config.sets as usize);
        for set_index in 0..self.config.sets {
            let set = run_set(
                self.factory,
                &self.config,
                &mut self.bots,
                self.echo_bot_output,
                set_index,
            )?;
            if let Some(hook) = &mut self.on_set {
                hook(&set)?;
            }
            sets.push(set);
        }
        let replay = Replay::new(self.match_id, self.config, self.bots.teams(), sets);
        if let Some(path) = &self.replay_path {
            write_replay(path, &replay)?;
        }
        let summary = summarize(&replay, self.replay_path.as_deref());
        if let Some(path) = &self.summary_path {
            write_summary(path, &summary)?;
        }
        Ok(MatchReport::new(replay, summary))
    }
}

fn run_set(
    factory: &GameFactory,
    config: &MatchConfig,
    bots: &mut BotRegistry,
    echo: bool,
    set_index: u32,
) -> Result<SetReplay, EngineError> {
    let teams = config.teams;
    let first_team = TeamId(set_index % teams);
    let seed = set_seed(config.seed, set_index);
    bots.begin_set(seed);
    let order = (0..teams)
        .map(|i| TeamId((first_team.0 + i) % teams))
        .collect();
    let setup = SetSetup::new(set_index, order, seed, config.game_config.clone());
    let mut game = factory(&setup)?;
    let initial_state = game.snapshot();
    let mut ticks: Vec<Tick> = Vec::new();

    let outcome = loop {
        if let GameStatus::Complete(outcome) = game.status() {
            break outcome;
        }
        let tick = ticks.len() as u32;
        if tick >= config.max_ticks {
            break game.tick_limit(config.max_ticks);
        }

        let schedule = game.schedule();
        check_schedule(&schedule, game.as_ref())?;
        let mut steps = Vec::new();
        for bot_id in schedule {
            if matches!(game.status(), GameStatus::Complete(_)) {
                break;
            }
            // Removed earlier this tick.
            let Some(team_id) = game.team_of(bot_id) else {
                continue;
            };
            let mut actions = Vec::new();
            let (result, team) = match bots.bot_mut(bot_id, team_id) {
                Ok(BotHandle { bot, team, seed }) => {
                    let mut ctx = StepCtx::new(
                        bot_id,
                        team,
                        set_index,
                        tick,
                        *seed,
                        game.as_mut(),
                        &mut actions,
                    );
                    (bot.step(&mut ctx), team.clone())
                }
                Err(BotLookupError::Game(error)) => return Err(error),
                Err(BotLookupError::Bot(failure)) => {
                    (StepResult::failed(failure), bots.team_info(team_id))
                }
            };
            let usage = Usage::new(result.time, result.memory);

            if echo && !result.stdout.is_empty() {
                for line in result.stdout.lines() {
                    eprintln!(
                        "[set {set_index} tick {tick} team {} bot {}] {line}",
                        team.name, bot_id
                    );
                }
            }

            let before = game.status();
            let failure = match result.outcome {
                Ok(()) => {
                    game.end_step(bot_id);
                    None
                }
                Err(failure) => {
                    game.bot_failed(bot_id, &failure);
                    Some(failure)
                }
            };
            if let GameStatus::Complete(outcome) = &before
                && game.status() != before
            {
                return Err(EngineError::Game(format!(
                    "game changed a completed outcome ({outcome:?}) after the step"
                )));
            }

            bots.retain(|id| game.team_of(id).is_some());
            let failure = failure.as_ref().map(BotFailure::record);
            steps.push(Step::new(
                bot_id,
                team_id,
                actions,
                result.stdout,
                failure,
                usage,
            ));
        }

        if !matches!(game.status(), GameStatus::Complete(_)) {
            game.end_tick();
        }
        ticks.push(Tick::new(tick, steps, game.snapshot()));
    };

    let result = SetResult::new(set_index, first_team, outcome, ticks.len() as u32);
    Ok(SetReplay::new(initial_state, ticks, result))
}

/// A schedule naming a bot the game does not have, or the same bot twice, is a game bug.
fn check_schedule(schedule: &[BotId], game: &dyn DynGame) -> Result<(), EngineError> {
    let mut seen = HashSet::with_capacity(schedule.len());
    for &bot in schedule {
        if game.team_of(bot).is_none() {
            return Err(EngineError::Game(format!("scheduled unknown bot {bot}")));
        }
        if !seen.insert(bot) {
            return Err(EngineError::Game(format!("scheduled bot {bot} twice")));
        }
    }
    Ok(())
}
