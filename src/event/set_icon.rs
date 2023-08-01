use super::util::{parse_first_element, MaybeInto, Parse};
use nvim_rs::Value;

/// Set the global window title
#[derive(Debug, Clone)]
pub struct SetIcon {
    pub title: String,
}

impl Parse for SetIcon {
    fn parse(value: Value) -> Option<Self> {
        Some(Self {
            title: parse_first_element(value)?.maybe_into()?,
        })
    }
}
