//! Login modal: email, then the emailed one-time code (or the login link).
//! The modal only collects input; the app runs the login and reports back.

use super::*;
use crate::tui::widgets::text::TextField;
use crossterm::event::KeyCode;

/// Which step the login is at.
#[derive(Debug, Clone, PartialEq)]
pub enum LoginStep {
    /// Typing the email.
    Email,
    /// Waiting for the site (the text says what for).
    Busy(String),
    /// Typing the code from the email.
    Code,
}

/// What the modal asks the app to do.
#[derive(Debug, Clone, PartialEq)]
pub enum LoginRequest {
    /// Start the login for this email (sends the code).
    SendEmail(String),
    /// Submit this code / link.
    SubmitCode(String),
}

/// Login state.
#[derive(Debug, Clone, PartialEq)]
pub struct LoginModal {
    /// Step.
    pub step: LoginStep,
    /// Email field.
    pub email: TextField,
    /// Code field.
    pub code: TextField,
    /// Last error.
    pub error: Option<String>,
    /// Request for the app (taken after each event).
    pub request: Option<LoginRequest>,
    /// Shown above the fields (e.g. "Log in to start the scan").
    pub reason: Option<String>,
}

impl LoginModal {
    /// Login with `email` pre-filled.
    pub fn new(email: Option<String>, reason: Option<String>) -> Self {
        LoginModal { step: LoginStep::Email, email: TextField::new(email.as_deref().unwrap_or("")),
            code: TextField::default(), error: None, request: None, reason }
    }
    /// The site sent the code: ask for it.
    pub fn need_code(&mut self) { self.step = LoginStep::Code; self.error = None; }
    /// The login failed: back to the step that can retry.
    pub fn failed(&mut self, e: &str) {
        self.error = Some(e.to_string());
        self.code.set("");
        self.step = LoginStep::Email;
    }
    fn submit(&mut self) {
        match self.step {
            LoginStep::Email => {
                let e = self.email.value.trim().to_string();
                if !e.contains('@') { self.error = Some("enter the email you log in to acp.us with".into()); return; }
                self.error = None;
                self.step = LoginStep::Busy(format!("Requesting a login code for {e}…"));
                self.request = Some(LoginRequest::SendEmail(e));
            }
            LoginStep::Code => {
                let c = self.code.value.trim().to_string();
                if c.is_empty() { self.error = Some("paste the code (or the login link) from the email".into()); return; }
                self.error = None;
                self.step = LoginStep::Busy("Verifying…".into());
                self.request = Some(LoginRequest::SubmitCode(c));
            }
            LoginStep::Busy(_) => {}
        }
    }
}

impl ModalState for LoginModal {
    fn title(&self) -> String { "Log in to acp.us".into() }
    fn on_key(&mut self, k: KeyEvent) -> Option<ModalResult> {
        match k.code {
            KeyCode::Esc => return Some(ModalResult::ClosedCancelled),
            KeyCode::Enter => self.submit(),
            _ => match self.step {
                LoginStep::Email => { self.email.on_key(k); }
                LoginStep::Code => { self.code.on_key(k); }
                LoginStep::Busy(_) => {}
            },
        }
        None
    }
    fn on_click(&mut self, h: ModalHit) -> Option<ModalResult> {
        match h {
            ModalHit::Button(0) => { self.submit(); None }
            ModalHit::Button(_) => Some(ModalResult::ClosedCancelled),
            _ => None,
        }
    }
}
