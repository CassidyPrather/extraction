//! The state machine: somewhere food goes in, is processed, and comes out.
//!
//! A [`Station`] is a chopping board, a pot, an oven, a crock on a shelf —
//! anything that runs one or more [`Process`]es. It has three states:
//!
//! ```text
//!            put / take                 work or tick            take
//!   ┌──────┐ ─────────► ┌──────┐  start  ┌─────────┐  done  ┌───────┐ ──► Idle
//!   │ Idle │            │ Idle │ ──────► │ Working │ ─────► │ Ready │
//!   └──────┘ ◄───────── └──────┘         └─────────┘        └───────┘
//!   (empty)               (loaded)        abort ──► Idle      heat: burns
//! ```
//!
//! - **Idle**: put food in (up to its capacity), or take the last thing back
//!   out. Nothing happens on its own.
//! - **Working**: locked. A physical station moves on with every
//!   [`Station::work`] (the first stroke starts it); a clock station moves
//!   on with every [`Station::tick`] once [`Station::start`]ed. Contents no
//!   recipe wants still finish — as [`Food::Mush`].
//! - **Ready**: holds one finished food until it is taken. On a heat
//!   station it keeps cooking: [`Event::Smoking`] halfway through its
//!   grace, then [`Food::Charcoal`].
//!
//! A clock station only moves when it is ticked, so whatever gates it — fuel
//! under a pot, power to an oven, the game being paused — is the caller's:
//! simply do not tick it. Everything that happens is also reported as an
//! [`Event`], for sound and motion, until [`Station::drain`] collects them.

use super::food::Food;
use super::process::{Drive, Process};
use super::recipe::{self, Recipe};

/// Where a station is in its cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Being filled, or empty.
    Idle,
    /// Under way. `recipe` is `None` when the contents are becoming mush.
    Working {
        process: Process,
        recipe: Option<&'static Recipe>,
        /// Strokes or ticks so far.
        progress: u32,
        /// Strokes or ticks to finish.
        goal: u32,
    },
    /// Finished, waiting to be taken.
    Ready {
        process: Process,
        food: Food,
        /// Ticks since it finished; heat burns it at its grace.
        over: u32,
    },
}

/// Something that happened at a station.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Food went in.
    Put(Food),
    /// Food came out.
    Took(Food),
    /// Under way, toward this food (`Mush` for no recipe).
    Began(Process, Food),
    /// One stroke of physical work.
    Stroke { progress: u32, goal: u32 },
    /// Finished.
    Done(Food),
    /// Heat: it will burn soon.
    Smoking,
    /// Heat: it burned.
    Burnt,
    /// Stopped part way; the contents are back as they were.
    Aborted,
}

/// Why a station would not do what was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It is holding all it can.
    Full,
    /// It is working, or holding something finished.
    Busy,
    /// There is nothing in it.
    Empty,
    /// Work on a clock station, or `start` on a physical one.
    WrongDrive,
}

/// One place food is processed.
#[derive(Clone, Debug)]
pub struct Station {
    processes: Vec<Process>,
    capacity: usize,
    contents: Vec<Food>,
    state: State,
    events: Vec<Event>,
}

impl Station {
    /// A station that runs `processes`. When its contents match recipes of
    /// more than one, the earlier process wins; when they match none, the
    /// first makes mush of them.
    ///
    /// # Panics
    /// If `processes` is empty, or mixes physical with clock-driven ones: a
    /// station is either worked or ticked.
    #[must_use]
    pub fn new(processes: &[Process]) -> Self {
        assert!(!processes.is_empty(), "a station needs a process");
        let drive = processes[0].drive();
        assert!(
            processes.iter().all(|p| p.drive() == drive),
            "a station is either worked or ticked, not both: {processes:?}"
        );
        Self {
            processes: processes.to_vec(),
            capacity: processes.iter().map(|p| p.capacity()).max().unwrap_or(1),
            contents: Vec::new(),
            state: State::Idle,
            events: Vec::new(),
        }
    }

    #[must_use]
    pub fn processes(&self) -> &[Process] {
        &self.processes
    }

    /// The most it holds at once.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Worked by hand, or run by the clock.
    #[must_use]
    pub fn drive(&self) -> Drive {
        self.processes[0].drive()
    }

    /// What is in it, in the order it went in. Empty once finished: the
    /// result is in [`State::Ready`].
    #[must_use]
    pub fn contents(&self) -> &[Food] {
        &self.contents
    }

    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }

    #[must_use]
    pub const fn is_idle(&self) -> bool {
        matches!(self.state, State::Idle)
    }

    /// The finished food, if there is one.
    #[must_use]
    pub const fn ready(&self) -> Option<Food> {
        match self.state {
            State::Ready { food, .. } => Some(food),
            _ => None,
        }
    }

    /// How far through the work it is, `0..=1`; 1 once finished.
    #[must_use]
    pub fn progress(&self) -> f32 {
        match self.state {
            State::Idle => 0.0,
            State::Working { progress, goal, .. } => progress as f32 / goal.max(1) as f32,
            State::Ready { .. } => 1.0,
        }
    }

    /// For heat, how close a finished food is to burning, `0..=1`.
    #[must_use]
    pub fn scorch(&self) -> f32 {
        match self.state {
            State::Ready { process, over, .. } if process.kind().burns() => {
                over as f32 / process.grace() as f32
            }
            _ => 0.0,
        }
    }

    /// What starting now would do: the process, and the recipe it would
    /// follow (`None` makes mush). `None` when empty or not idle.
    #[must_use]
    pub fn plan(&self) -> Option<(Process, Option<&'static Recipe>)> {
        if !self.is_idle() || self.contents.is_empty() {
            return None;
        }
        Some(
            self.processes
                .iter()
                .find_map(|&p| recipe::find(p, &self.contents).map(|r| (p, Some(r))))
                .unwrap_or((self.processes[0], None)),
        )
    }

    /// Recipes the current contents are on the way to, for hints.
    pub fn toward(&self) -> impl Iterator<Item = &'static Recipe> + '_ {
        let idle = self.is_idle();
        recipe::toward(&self.processes, &self.contents).filter(move |_| idle)
    }

    /// Whether there is room to put something in now. Every food is
    /// welcome; a game that wants its pot to turn a cake away checks that
    /// itself before calling [`Station::put`].
    pub fn room(&self) -> Result<(), Refusal> {
        if !self.is_idle() {
            Err(Refusal::Busy)
        } else if self.contents.len() >= self.capacity {
            Err(Refusal::Full)
        } else {
            Ok(())
        }
    }

    /// Put `food` in.
    pub fn put(&mut self, food: Food) -> Result<(), Refusal> {
        self.room()?;
        self.contents.push(food);
        self.events.push(Event::Put(food));
        Ok(())
    }

    /// What [`Station::take`] would hand over.
    #[must_use]
    pub fn peek(&self) -> Option<Food> {
        match self.state {
            State::Ready { food, .. } => Some(food),
            State::Idle => self.contents.last().copied(),
            State::Working { .. } => None,
        }
    }

    /// Take the finished food out, or, while idle, the last thing put in.
    pub fn take(&mut self) -> Result<Food, Refusal> {
        let food = match self.state {
            State::Ready { food, .. } => {
                self.state = State::Idle;
                food
            }
            State::Idle => self.contents.pop().ok_or(Refusal::Empty)?,
            State::Working { .. } => return Err(Refusal::Busy),
        };
        self.events.push(Event::Took(food));
        Ok(food)
    }

    /// Start a clock station on what is in it.
    pub fn start(&mut self) -> Result<(), Refusal> {
        if self.drive() != Drive::Clock {
            return Err(Refusal::WrongDrive);
        }
        self.begin()
    }

    fn begin(&mut self) -> Result<(), Refusal> {
        if !self.is_idle() {
            return Err(Refusal::Busy);
        }
        let (process, recipe) = self.plan().ok_or(Refusal::Empty)?;
        let goal = recipe.map_or_else(|| process.mush_work(), |r| r.work);
        let food = recipe.map_or(Food::Mush, |r| r.output);
        self.state = State::Working {
            process,
            recipe,
            progress: 0,
            goal,
        };
        self.events.push(Event::Began(process, food));
        Ok(())
    }

    /// One stroke of work on a physical station. The first starts it.
    pub fn work(&mut self) -> Result<(), Refusal> {
        if self.drive() != Drive::Effort {
            return Err(Refusal::WrongDrive);
        }
        if self.is_idle() {
            self.begin()?;
        }
        match &mut self.state {
            State::Working { progress, goal, .. } => {
                *progress += 1;
                let (progress, goal) = (*progress, *goal);
                self.events.push(Event::Stroke { progress, goal });
                if progress >= goal {
                    self.finish();
                }
                Ok(())
            }
            _ => Err(Refusal::Busy),
        }
    }

    /// One tick of the clock. Moves a started clock station along, and lets
    /// a finished dish on a heat station cook on toward burning. Does
    /// nothing to physical stations.
    pub fn tick(&mut self) {
        if self.drive() != Drive::Clock {
            return;
        }
        match &mut self.state {
            State::Idle => {}
            State::Working { progress, goal, .. } => {
                *progress += 1;
                if *progress >= *goal {
                    self.finish();
                }
            }
            State::Ready {
                process,
                food,
                over,
            } => {
                if !process.kind().burns() || *food == Food::Charcoal {
                    return;
                }
                *over += 1;
                if *over == process.warning() {
                    self.events.push(Event::Smoking);
                }
                if *over >= process.grace() {
                    *food = Food::Charcoal;
                    self.events.push(Event::Burnt);
                }
            }
        }
    }

    fn finish(&mut self) {
        if let State::Working {
            process, recipe, ..
        } = self.state
        {
            let food = recipe.map_or(Food::Mush, |r| r.output);
            self.contents.clear();
            self.state = State::Ready {
                process,
                food,
                over: 0,
            };
            self.events.push(Event::Done(food));
        }
    }

    /// Stop part way, leaving the contents as they went in. Progress is
    /// lost. Does nothing unless working.
    pub fn abort(&mut self) {
        if matches!(self.state, State::Working { .. }) {
            self.state = State::Idle;
            self.events.push(Event::Aborted);
        }
    }

    /// Everything that has happened since the last drain.
    pub fn drain(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::super::secs;
    use super::*;

    fn ticks(station: &mut Station, n: u32) {
        for _ in 0..n {
            station.tick();
        }
    }

    #[test]
    fn a_board_chops_a_carrot_stroke_by_stroke() {
        let mut board = Station::new(&[Process::Chop]);
        assert_eq!(board.capacity(), 1);
        board.put(Food::Carrot).unwrap();
        assert_eq!(board.put(Food::Onion), Err(Refusal::Full));
        assert_eq!(board.start(), Err(Refusal::WrongDrive));
        for _ in 0..3 {
            board.work().unwrap();
        }
        assert!(board.ready().is_none());
        assert_eq!(board.take(), Err(Refusal::Busy), "took from a busy board");
        board.work().unwrap();
        assert_eq!(board.ready(), Some(Food::ChoppedCarrot));
        assert_eq!(board.take(), Ok(Food::ChoppedCarrot));
        assert!(board.is_idle() && board.contents().is_empty());
        let events = board.drain();
        assert_eq!(events.first(), Some(&Event::Put(Food::Carrot)));
        assert!(events.contains(&Event::Began(Process::Chop, Food::ChoppedCarrot)));
        assert!(events.contains(&Event::Done(Food::ChoppedCarrot)));
        assert!(board.drain().is_empty(), "draining twice repeats events");
    }

    #[test]
    fn a_pot_boils_then_smokes_then_burns() {
        let mut pot = Station::new(&[Process::Boil]);
        pot.put(Food::Water).unwrap();
        pot.put(Food::Egg).unwrap();
        // Ticks before starting do nothing.
        ticks(&mut pot, 100);
        assert!(pot.is_idle());
        pot.start().unwrap();
        assert_eq!(pot.put(Food::Salt), Err(Refusal::Busy));
        ticks(&mut pot, secs(8));
        assert_eq!(pot.ready(), Some(Food::BoiledEgg));
        pot.drain();
        ticks(&mut pot, Process::Boil.warning());
        assert_eq!(pot.drain(), vec![Event::Smoking]);
        ticks(&mut pot, Process::Boil.grace() - Process::Boil.warning());
        assert_eq!(pot.ready(), Some(Food::Charcoal));
        assert_eq!(pot.drain(), vec![Event::Burnt]);
        ticks(&mut pot, 1000);
        assert!(pot.drain().is_empty(), "charcoal burned again");
    }

    #[test]
    fn time_waits_for_you() {
        let mut crock = Station::new(&[Process::Prove, Process::Soak, Process::Steep]);
        crock.put(Food::Dough).unwrap();
        crock.start().unwrap();
        ticks(&mut crock, secs(20));
        assert_eq!(crock.ready(), Some(Food::RisenDough));
        ticks(&mut crock, secs(600));
        assert_eq!(crock.ready(), Some(Food::RisenDough), "time spoiled it");
    }

    #[test]
    fn a_station_picks_the_process_its_contents_call_for() {
        let mut crock = Station::new(&[Process::Prove, Process::Soak, Process::Steep]);
        crock.put(Food::TeaLeaf).unwrap();
        crock.put(Food::HotWater).unwrap();
        assert_eq!(crock.plan().map(|(p, _)| p), Some(Process::Steep));
        while crock.take().is_ok() {}
        crock.put(Food::Water).unwrap();
        crock.put(Food::Beans).unwrap();
        assert_eq!(crock.plan().map(|(p, _)| p), Some(Process::Soak));
        // Nothing wants beans alone, so the first process makes mush.
        crock.take().unwrap();
        crock.take().unwrap();
        crock.put(Food::Beans).unwrap();
        assert_eq!(crock.plan(), Some((Process::Prove, None)));
    }

    #[test]
    fn nonsense_still_cooks_into_mush() {
        let mut pan = Station::new(&[Process::Fry]);
        pan.put(Food::Sugar).unwrap();
        pan.put(Food::Tea).unwrap();
        assert_eq!(pan.plan(), Some((Process::Fry, None)));
        pan.start().unwrap();
        ticks(&mut pan, Process::Fry.mush_work());
        assert_eq!(pan.take(), Ok(Food::Mush));
    }

    #[test]
    fn idle_contents_come_back_out_last_in_first() {
        let mut bowl = Station::new(&[Process::Knead, Process::Mix]);
        assert_eq!(bowl.take(), Err(Refusal::Empty));
        bowl.put(Food::Flour).unwrap();
        bowl.put(Food::Egg).unwrap();
        assert_eq!(bowl.peek(), Some(Food::Egg));
        assert_eq!(bowl.take(), Ok(Food::Egg));
        assert_eq!(bowl.contents(), &[Food::Flour]);
    }

    #[test]
    fn aborting_gives_the_contents_back() {
        let mut oven = Station::new(&[Process::Bake]);
        oven.put(Food::Dough).unwrap();
        oven.start().unwrap();
        ticks(&mut oven, 5);
        oven.abort();
        assert!(oven.is_idle());
        assert_eq!(oven.contents(), &[Food::Dough]);
        assert!(oven.progress().abs() < f32::EPSILON);
    }

    #[test]
    fn starting_an_empty_station_is_refused() {
        let mut oven = Station::new(&[Process::Bake]);
        assert_eq!(oven.start(), Err(Refusal::Empty));
        let mut board = Station::new(&[Process::Chop]);
        assert_eq!(board.work(), Err(Refusal::Empty));
    }

    #[test]
    #[should_panic(expected = "worked or ticked")]
    fn a_station_cannot_be_both_worked_and_ticked() {
        let _ = Station::new(&[Process::Chop, Process::Boil]);
    }

    #[test]
    fn capacity_is_the_biggest_recipe_it_runs() {
        assert_eq!(Station::new(&[Process::Boil]).capacity(), 4);
        assert_eq!(Station::new(&[Process::Knead, Process::Mix]).capacity(), 3);
    }
}
