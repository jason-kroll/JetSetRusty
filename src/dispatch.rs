// Typed screen state machine — the replacement for the C-era fn-pointer
// dispatcher (the EVENT globals). The whole dispatch state is one Copy struct
// in a single thread-local Cell: main-thread only (the SDL audio callback
// never touches it), reads copy out and writes copy in, so no RefCell is
// needed.
//
// Timing contract (pinned by the tests below and by the golden replays):
// `pending` is consumed once, at the top of a frame (game_frame's step 1);
// `screen` writes are visible to the next handler read, which may be later
// in the same frame — a write made from a responder is seen by the same
// frame's ticker/drawer, while a queued pending only fires on the next frame.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoaderPhase {
    /// drawer1 runs once, then Bar
    Enter,
    /// drawer2 draws the loading bar until the loader music stops, then Ready
    Bar,
    /// responder, ticker and drawer3 installed
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TitlePhase {
    /// do_title_init runs once (draws the screen), then Scroll
    Init,
    Scroll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GamePhase {
    /// game_init_room runs once as the ticker, then Play (or PausedDrawOnce
    /// when paused)
    Init,
    Play,
    /// uncheated pause: pause ticker and drawer
    Pause,
    /// cheated pause (or after PausedDrawOnce has fired): no ticker, no drawer
    PauseCheat,
    /// game_init_room ran while paused: the game drawer runs once, then
    /// PauseCheat
    PausedDrawOnce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiePhase {
    /// die_init runs once, then Anim
    Init,
    Anim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameOverPhase {
    /// gameover_init runs once, then Run
    Init,
    Run,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Screen {
    Loader(LoaderPhase),
    Codes,
    Title(TitlePhase),
    Game(GamePhase),
    Die(DiePhase),
    GameOver(GameOverPhase),
}

/// A one-shot command queued for the start of the next frame — the old
/// ACTION slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    /// Install the screen at its initial phase and run its enter work.
    Enter(Screen),
    /// Title's game_start: reset the game, then install Game(Init).
    StartGame,
    /// game_init_room one-shot (cheat level select).
    ReinitRoom,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Dispatch {
    pub screen: Screen,
    pub pending: Option<Pending>,
}

thread_local! {
    static DISPATCH: Cell<Dispatch> = const {
        Cell::new(Dispatch {
            screen: Screen::Loader(LoaderPhase::Enter),
            pending: Some(Pending::Enter(Screen::Loader(LoaderPhase::Enter))),
        })
    };
}

pub(crate) fn dispatch() -> Dispatch {
    DISPATCH.with(Cell::get)
}

pub(crate) fn with_dispatch<R>(f: impl FnOnce(&mut Dispatch) -> R) -> R {
    DISPATCH.with(|c| {
        let mut d = c.get();
        let r = f(&mut d);
        c.set(d);
        r
    })
}

/// Queue a one-shot command for the start of the next frame (the C-era
/// ACTION slot).
pub(crate) fn queue_pending(p: Pending) {
    with_dispatch(|d| d.pending = Some(p));
}

/// Install a screen/phase now — visible to the next handler read, which may
/// be later in the same frame (the C-era TICKER/DRAWER/RESPONDER writes).
pub(crate) fn set_screen(s: Screen) {
    with_dispatch(|d| d.screen = s);
}

/// Consume and run the queued pending command, if any (game_frame step 1).
pub(crate) fn run_pending() {
    let p = with_dispatch(|d| d.pending.take());
    let Some(p) = p else { return };
    match p {
        Pending::Enter(s) => {
            set_screen(s);
            if let Screen::Codes = s {
                crate::codes::codes_enter();
            }
        }
        Pending::StartGame => crate::title::game_start(),
        Pending::ReinitRoom => {
            let mut miner = crate::game::GAME_STATE.miner.lock().unwrap();
            crate::game::game_init_room(&mut miner);
        }
    }
}

/// Run the current screen's responder (game_frame step 2, once per input).
pub(crate) fn run_responder() {
    match dispatch().screen {
        Screen::Loader(LoaderPhase::Ready) => crate::loader::do_loader_responder(),
        Screen::Codes => crate::codes::do_codes_responder(),
        Screen::Title(_) => crate::title::do_title_responder(),
        Screen::Game(_) => crate::game::do_game_responder(),
        Screen::Loader(LoaderPhase::Enter | LoaderPhase::Bar)
        | Screen::Die(_)
        | Screen::GameOver(_) => {}
    }
}

/// Run the current screen's ticker (game_frame step 4).
pub(crate) fn run_ticker() {
    match dispatch().screen {
        Screen::Loader(LoaderPhase::Ready) => crate::loader::do_loader_ticker(),
        Screen::Codes => crate::codes::do_codes_ticker(),
        Screen::Title(TitlePhase::Init) => crate::title::do_title_init(),
        Screen::Title(TitlePhase::Scroll) => crate::title::do_title_ticker(),
        Screen::Game(GamePhase::Init) => {
            let mut miner = crate::game::GAME_STATE.miner.lock().unwrap();
            crate::game::game_init_room(&mut miner);
        }
        Screen::Game(GamePhase::Play) => crate::game::do_game_ticker(),
        Screen::Game(GamePhase::Pause) => crate::game::do_pause_ticker(),
        Screen::Die(DiePhase::Init) => crate::die::die_init(),
        Screen::Die(DiePhase::Anim) => crate::die::die_ticker(),
        Screen::GameOver(GameOverPhase::Init) => crate::gameover::gameover_init(),
        Screen::GameOver(GameOverPhase::Run) => crate::gameover::gameover_ticker(),
        Screen::Loader(LoaderPhase::Enter | LoaderPhase::Bar)
        | Screen::Game(GamePhase::PauseCheat | GamePhase::PausedDrawOnce) => {}
    }
}

/// Run the current screen's drawer (game_frame step 5).
pub(crate) fn run_drawer() {
    match dispatch().screen {
        Screen::Loader(LoaderPhase::Enter) => crate::loader::do_loader_drawer1(),
        Screen::Loader(LoaderPhase::Bar) => crate::loader::do_loader_drawer2(),
        Screen::Loader(LoaderPhase::Ready) => crate::loader::do_loader_drawer3(),
        Screen::Codes => crate::codes::do_codes_drawer(),
        Screen::Title(_) => crate::title::do_title_drawer(),
        Screen::Game(GamePhase::Init | GamePhase::Play) => crate::game::do_game_drawer(),
        Screen::Game(GamePhase::Pause) => crate::game::do_pause_drawer(),
        Screen::Game(GamePhase::PausedDrawOnce) => crate::game::do_draw_once(),
        Screen::Die(_) => crate::die::die_drawer(),
        Screen::GameOver(_) => crate::gameover::gameover_drawer(),
        Screen::Game(GamePhase::PauseCheat) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each test runs on its own thread, so the thread-local DISPATCH starts
    // at its initial value in every test.

    #[test]
    fn initial_state_queues_the_loader_enter() {
        assert_eq!(
            dispatch().pending,
            Some(Pending::Enter(Screen::Loader(LoaderPhase::Enter)))
        );
        assert_eq!(dispatch().screen, Screen::Loader(LoaderPhase::Enter));
    }

    #[test]
    fn pending_is_consumed_once_at_run_pending() {
        with_dispatch(|d| {
            d.screen = Screen::Codes;
            d.pending = None;
        });
        queue_pending(Pending::Enter(Screen::Die(DiePhase::Init)));
        // Queued, but not yet visible as the screen: pending fires at the
        // next frame's step 1, not at queue time.
        assert_eq!(dispatch().screen, Screen::Codes);
        run_pending();
        assert_eq!(dispatch().screen, Screen::Die(DiePhase::Init));
        assert_eq!(dispatch().pending, None);
        // Consumed: a second run_pending does not re-fire the enter.
        run_pending();
        assert_eq!(dispatch().screen, Screen::Die(DiePhase::Init));
        assert_eq!(dispatch().pending, None);
    }

    #[test]
    fn set_screen_is_visible_to_the_next_handler_read_in_the_same_frame() {
        with_dispatch(|d| {
            d.screen = Screen::Game(GamePhase::Play);
            d.pending = None;
        });
        // A responder-phase write (game_pause does this) must be seen by the
        // same frame's ticker/drawer reads: dispatch() is a fresh copy at
        // every read point.
        set_screen(Screen::Game(GamePhase::Pause));
        assert_eq!(dispatch().screen, Screen::Game(GamePhase::Pause));
    }

    #[test]
    fn paused_draw_once_drawer_fires_once_then_clears_to_pause_cheat() {
        // The draw body writes the shared video buffer, which parallel test
        // threads may also be drawing into — clear the frame flag so
        // do_game_drawer early-returns and this test isolates just the
        // once-gate (the real draw path is covered by the pause-level
        // golden scenario).
        crate::game::GAME_STATE
            .frame
            .store(false, std::sync::atomic::Ordering::Relaxed);
        with_dispatch(|d| {
            d.screen = Screen::Game(GamePhase::PausedDrawOnce);
            d.pending = None;
        });
        run_drawer();
        assert_eq!(dispatch().screen, Screen::Game(GamePhase::PauseCheat));
        // The one-shot is consumed: the next drawer read is a no-op phase.
        run_drawer();
        assert_eq!(dispatch().screen, Screen::Game(GamePhase::PauseCheat));
    }
}
