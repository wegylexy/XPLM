//! Commands (`XPLMUtilities.h`'s command subsystem). `Command` is a thin
//! handle (`XPLMCommandRef` is never owned by any one plugin — it outlives
//! whichever plugin created it); `CommandHandler` is the RAII + trampoline
//! wrapper, same shape as `FlightLoop`/`Menu`/etc.

use std::ffi::{CString, c_void};
use std::os::raw::c_int;

use xplm_sys::{
    XPLMCommandBegin, XPLMCommandEnd, XPLMCommandOnce, XPLMCommandPhase, XPLMCommandRef,
    XPLMCreateCommand, XPLMFindCommand, XPLMRegisterCommandHandler, XPLMUnregisterCommandHandler,
    xplm_CommandBegin, xplm_CommandEnd,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandPhase {
    Begin,
    Continue,
    End,
}

impl From<XPLMCommandPhase> for CommandPhase {
    fn from(raw: XPLMCommandPhase) -> Self {
        if raw == xplm_CommandBegin {
            CommandPhase::Begin
        } else if raw == xplm_CommandEnd {
            CommandPhase::End
        } else {
            CommandPhase::Continue
        }
    }
}

/// A handle to an X-Plane command, found or created by name. Commands are
/// never "owned" by a plugin — unlike every other handle in this crate,
/// there's nothing to `Drop`; the command reference stays valid (and usable
/// by other plugins) even after yours unloads.
#[derive(Clone, Copy)]
pub struct Command(XPLMCommandRef);

impl Command {
    /// Looks up an existing command by name. Returns `None` if no command
    /// with that name has been created yet.
    pub fn find(name: &str) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let raw = unsafe { XPLMFindCommand(c_name.as_ptr()) };
        (!raw.is_null()).then_some(Self(raw))
    }

    /// Creates a new command, or returns the existing one if `name` is
    /// already a registered command (`description` is only used the first
    /// time).
    pub fn create(name: &str, description: &str) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let c_description = CString::new(description).ok()?;
        let raw = unsafe { XPLMCreateCommand(c_name.as_ptr(), c_description.as_ptr()) };
        (!raw.is_null()).then_some(Self(raw))
    }

    /// Starts the command; must be balanced by [`Command::end`].
    pub fn begin(&self) {
        unsafe { XPLMCommandBegin(self.0) }
    }

    /// Ends a command previously started with [`Command::begin`].
    pub fn end(&self) {
        unsafe { XPLMCommandEnd(self.0) }
    }

    /// Begins and ends the command immediately.
    pub fn once(&self) {
        unsafe { XPLMCommandOnce(self.0) }
    }

    /// Registers `callback` to run whenever this command executes.
    /// `before = true` runs it before X-Plane's own handling (returning
    /// `false` suppresses X-Plane's handling); `before = false` runs it
    /// after. Return `true` from `callback` to let processing continue.
    pub fn register_handler(
        &self,
        before: bool,
        callback: impl FnMut(CommandPhase) -> bool + 'static,
    ) -> CommandHandler {
        CommandHandler::register(*self, before, callback)
    }
}

type Callback = dyn FnMut(CommandPhase) -> bool + 'static;

/// A registered command handler. Dropping it unregisters the callback and
/// frees the closure — the command itself (see [`Command`]'s docs) is
/// unaffected, since it isn't owned by this registration.
pub struct CommandHandler {
    command: XPLMCommandRef,
    before: c_int,
    refcon: *mut Box<Callback>,
}

unsafe impl Send for CommandHandler {} // see FlightLoop's identical rationale.

impl CommandHandler {
    fn register(
        command: Command,
        before: bool,
        callback: impl FnMut(CommandPhase) -> bool + 'static,
    ) -> Self {
        let boxed: Box<Callback> = Box::new(callback);
        let refcon = Box::into_raw(Box::new(boxed));
        let before = before as c_int;
        unsafe {
            XPLMRegisterCommandHandler(command.0, Some(trampoline), before, refcon as *mut c_void);
        }
        Self {
            command: command.0,
            before,
            refcon,
        }
    }
}

impl Drop for CommandHandler {
    fn drop(&mut self) {
        unsafe {
            // Must match the (command, fn ptr, before, refcon) tuple passed
            // to XPLMRegisterCommandHandler exactly.
            XPLMUnregisterCommandHandler(
                self.command,
                Some(trampoline),
                self.before,
                self.refcon as *mut c_void,
            );
            drop(Box::from_raw(self.refcon));
        }
    }
}

unsafe extern "C" fn trampoline(
    _command: XPLMCommandRef,
    phase: XPLMCommandPhase,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let callback: &mut Callback = unsafe { &mut *(*(refcon as *mut Box<Callback>)) };
        callback(phase.into())
    })
    .unwrap_or(true) as c_int
}
