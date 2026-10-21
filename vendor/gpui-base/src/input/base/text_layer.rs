//! NOSHAPE PATCH (agent qa4): the text of a multi-line input as its own cached
//! view. It re-renders when the input notifies (an edit, a cursor move, a
//! scroll), not when the caret blinks: the blink notifies only the caret view
//! (`BlinkCursor`), which the input renders beside this one.

use gpui::{Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Subscription, WeakEntity, Window, div};

use super::{InputBaseState, element::TextElement, kind::InputModeKind};

pub(crate) struct TextLayer<M: InputModeKind> {
    state: WeakEntity<InputBaseState<M>>,
    _observe: Subscription,
}

impl<M: InputModeKind> TextLayer<M> {
    pub(crate) fn new(state: &Entity<InputBaseState<M>>, cx: &mut Context<Self>) -> Self {
        Self { state: state.downgrade(), _observe: cx.observe(state, |_, _, cx| cx.notify()) }
    }
}

impl<M: InputModeKind> Render for TextLayer<M> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.state.upgrade() {
            Some(state) => {
                let placeholder = state.read(cx).placeholder.clone();
                div().size_full().child(TextElement::new(state).placeholder(placeholder))
            }
            None => div(),
        }
    }
}
