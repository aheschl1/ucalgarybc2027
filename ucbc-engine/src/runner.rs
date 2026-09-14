//! Drives a match: sets, ticks, steps, replay assembly.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::bot::TeamSpec;
use crate::bot::registry::{BotHandle, BotRegistry};
use crate::error::{BotFailure, EngineError};
use crate::game::{GameFactory, GameRegistry, GameStatus, Outcome, SetSetup};
use crate::ids::{BotRef, TeamId};
use crate::replay::{MatchConfig, Replay, SetReplay, SetResult, Step, Tick, write_replay};
use crate::rng::set_seed;
use crate::step::{StepCtx, StepResult};
use crate::summary::{Summary, summarize, write_summary};

pub struct MatchSpec {
    match_id: String,
    config: MatchConfig,
    teams: Vec<TeamSpec>,
    replay_path: Option<PathBuf>,
    summary_path: Option<PathBuf>,
    echo_bot_output: bool,
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
        let bots: BotRegistry = BotRegistry::new(spec.teams);
        Ok(Self {
            factory,
            match_id: spec.match_id,
            config,
            bots,
            replay_path: spec.replay_path,
            summary_path: spec.summary_path,
            echo_bot_output: spec.echo_bot_output,
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
    let setup = SetSetup::new(
        set_index,
        teams,
        first_team,
        seed,
        config.game_config.clone(),
    );
    let mut game = factory(&setup)?;
    let initial_state = game.snapshot();
    let mut ticks: Vec<Tick> = Vec::new();

    let outcome = loop {
        if let GameStatus::Complete(outcome) = game.status() {
            break outcome;
        }
        let tick = ticks.len() as u32;
        if tick >= config.max_ticks {
            break Outcome::draw(format!("tick limit of {} reached", config.max_ticks));
        }

        let schedule = game.schedule();
        check_schedule(&schedule, bots)?;
        let mut steps = Vec::new();
        for bot_ref in schedule {
            if matches!(game.status(), GameStatus::Complete(_)) {
                break;
            }
            if bots.is_dead(bot_ref.id) {
                continue;
            }
            let mut actions = Vec::new();
            let (result, team) = match bots.bot_mut(bot_ref) {
                Ok(BotHandle { bot, team, seed }) => {
                    let mut ctx = StepCtx::new(
                        bot_ref,
                        team,
                        set_index,
                        tick,
                        *seed,
                        game.as_mut(),
                        &mut actions,
                    );
                    (bot.step(&mut ctx), team.clone())
                }
                Err(failure) => (StepResult::failed(failure), bots.team_info(bot_ref.team)),
            };

            if echo && !result.stdout.is_empty() {
                for line in result.stdout.lines() {
                    eprintln!(
                        "[set {set_index} tick {tick} team {} bot {}] {line}",
                        team.name, bot_ref.id
                    );
                }
            }

            let failure = match result.outcome {
                Ok(()) => {
                    game.end_step(bot_ref);
                    None
                }
                Err(failure) => {
                    bots.despawn(bot_ref.id);
                    game.bot_failed(bot_ref, &failure);
                    Some(failure)
                }
            };

            for id in game.despawned() {
                bots.despawn(id);
            }
            let failure = failure.as_ref().map(BotFailure::record);
            steps.push(Step::new(bot_ref, actions, result.stdout, failure));
        }

        if !matches!(game.status(), GameStatus::Complete(_)) {
            game.end_tick();
        }
        ticks.push(Tick::new(tick, steps, game.snapshot()));
    };

    let result = SetResult::new(set_index, first_team, outcome, ticks.len() as u32);
    Ok(SetReplay::new(initial_state, ticks, result))
}

/// A schedule naming an unknown team, a dead bot, or the same bot twice is a game bug.
fn check_schedule(schedule: &[BotRef], bots: &BotRegistry) -> Result<(), EngineError> {
    let mut seen = HashSet::with_capacity(schedule.len());
    for bot in schedule {
        if !bots.has_team(bot.team) {
            return Err(EngineError::Game(format!(
                "scheduled bot {} for unknown team {}",
                bot.id, bot.team
            )));
        }
        if bots.is_dead(bot.id) {
            return Err(EngineError::Game(format!("scheduled dead bot {}", bot.id)));
        }
        if !seen.insert(bot.id) {
            return Err(EngineError::Game(format!("scheduled bot {} twice", bot.id)));
        }
    }
    Ok(())
}
