//! Masking and unmasking a field when its `secure` attribute changes.
//!
//! The attribute hook queues each field whose `secure` attribute was written. The frame drains the
//! queue after the commands and before the restyle, so a field's text is masked before anything
//! styles, shapes or paints it. The attribute is read again here: the queue says *which* fields to
//! look at, and the document says what they are now.

use crate::window::Window;

impl Window {
    /// Masks or unmasks every field whose `secure` attribute changed since the last frame.
    pub(crate) fn apply_secure_changes(&mut self) {
        if self.secure_changes.borrow().is_empty() {
            return;
        }
        let changed = core::mem::take(&mut *self.secure_changes.borrow_mut());
        let focused = self.router.interaction().focus.focused();
        let mut focus_changed = false;
        for node in changed {
            let document = self.document.borrow();
            let on = crate::editing::Editors::is_secure_element(&document, node);
            if self.editors.set_secure(&document, node, on) && Some(node) == focused {
                focus_changed = true;
            }
        }
        if focus_changed {
            // The caret moves with the text it is in, and an input method is told the field's new
            // purpose.
            self.carets.restart(self.clock.now());
            self.report_caret();
        }
    }
}
